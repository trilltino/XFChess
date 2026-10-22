//! The tournament browser: status filters, one card per tournament (details,
//! games to watch or replay, registration, the waiting room), private invite
//! codes, and the player's own progress once registered.

use super::*;

#[cfg(feature = "solana")]
use crate::multiplayer::network::vps::{PlayerState, TournamentSummary};
#[cfg(feature = "solana")]
use crate::multiplayer::solana::tournament::{TournamentClientState, TournamentJoinStatus};

/// A finished game the player asked to replay: id, white, black.
#[cfg(feature = "solana")]
type ReplayRequest = (u64, String, String);

pub(in super::super) fn render_tournament_browser_screen(
    ui: &mut egui::Ui,
    ctx: &mut MainMenuUIContext,
) {
    ctx.learn_viewport.rect_px = None;
    // Set by a Finished tab's Replay button; acted on after the UI closure,
    // where `ctx` is no longer borrowed by egui.
    #[cfg(feature = "solana")]
    let mut replay_request: Option<ReplayRequest> = None;
    ui.vertical_centered(|ui| {
        ui.heading(
            egui::RichText::new("TOURNAMENTS")
                .size(24.0)
                .color(egui::Color32::from_rgb(255, 200, 50))
                .strong(),
        );
        ui.add_space(8.0);
        if ui.button("‹ Back").clicked() {
            ctx.menu_state.set(crate::core::MenuState::Main);
        }
        ui.add_space(16.0);

        #[cfg(feature = "solana")]
        {
            render_browser(ui, ctx, &mut replay_request);
            if let Some(tc) = ctx.tournament_client.as_deref_mut() {
                render_private_code_join(ui, tc);
            }
        }
        #[cfg(not(feature = "solana"))]
        ui.label(
            egui::RichText::new("Tournament browser requires the solana feature.")
                .size(13.0)
                .color(egui::Color32::GRAY)
                .italics(),
        );
    });

    #[cfg(feature = "solana")]
    if let Some(request) = replay_request {
        start_replay(ctx, request);
    }
}

#[cfg(feature = "solana")]
fn render_browser(
    ui: &mut egui::Ui,
    ctx: &mut MainMenuUIContext,
    replay_request: &mut Option<ReplayRequest>,
) {
    let wallet_pubkey = ctx
        .solana_state
        .as_ref()
        .and_then(|s| s.wallet_pubkey.map(|p| p.to_string()));

    render_status_bar(ui, ctx);
    render_filter_chips(ui, ctx);

    let tournaments = ctx
        .tournament_client
        .as_ref()
        .map(|tc| tc.available_tournaments.clone())
        .unwrap_or_default();
    ui.horizontal_wrapped(|ui| {
        ui.label(
            egui::RichText::new(format!(
                "Backend: {}",
                crate::multiplayer::network::vps::vps_base()
            ))
            .size(10.0)
            .color(egui::Color32::from_gray(120)),
        );
        ui.label(
            egui::RichText::new(format!("Fetched: {}", tournaments.len()))
                .size(10.0)
                .color(if tournaments.is_empty() {
                    egui::Color32::from_rgb(230, 160, 80)
                } else {
                    egui::Color32::from_rgb(100, 200, 140)
                }),
        );
    });
    ui.add_space(4.0);

    // Filter by the selected status, and drop anything the player dismissed.
    let filter_key = ctx
        .tournament_client
        .as_ref()
        .and_then(|tc| tc.status_filter.clone());
    let dismissed_ids = ctx
        .tournament_client
        .as_ref()
        .map(|tc| tc.dismissed_ids.clone())
        .unwrap_or_default();
    let visible: Vec<_> = tournaments
        .iter()
        .filter(|t| {
            !dismissed_ids.contains(&t.tournament_id)
                && filter_key
                    .as_ref()
                    .is_none_or(|k| t.status.to_lowercase().contains(k.as_str()))
        })
        .collect();

    if let Some(tc) = ctx.tournament_client.as_deref_mut() {
        render_my_progress(ui, tc);
    }

    if visible.is_empty() {
        let message = if tournaments.is_empty() {
            "No tournaments available. Check back later."
        } else {
            "No tournaments match the current filter."
        };
        ui.label(
            egui::RichText::new(message)
                .size(13.0)
                .color(egui::Color32::GRAY)
                .italics(),
        );
        let last_error = ctx
            .tournament_client
            .as_ref()
            .and_then(|tc| tc.last_poll_error.clone())
            .unwrap_or_else(|| "none".to_string());
        ui.label(
            egui::RichText::new(format!(
                "Diagnostics: backend={} | fetched={} | visible={} | filter={} | last_error={}",
                crate::multiplayer::network::vps::vps_base(),
                tournaments.len(),
                visible.len(),
                filter_key.as_deref().unwrap_or("all"),
                last_error
            ))
            .size(10.5)
            .color(egui::Color32::from_gray(120)),
        );
        return;
    }

    egui::ScrollArea::vertical()
        .max_height(400.0)
        .show(ui, |ui| {
            for t in visible {
                render_tournament_card(ui, ctx, t, wallet_pubkey.as_deref(), replay_request);
                ui.add_space(4.0);
            }
        });
}

/// Refreshing / last-updated, a refresh button, and the last poll error.
#[cfg(feature = "solana")]
fn render_status_bar(ui: &mut egui::Ui, ctx: &mut MainMenuUIContext) {
    let Some(tc) = ctx.tournament_client.as_deref_mut() else {
        ui.add_space(4.0);
        return;
    };
    let is_refreshing = tc.list_rx.is_some();
    let last_poll_age = tc.last_list_poll.map(|t| t.elapsed().as_secs());
    ui.horizontal(|ui| {
        if is_refreshing {
            ui.label(
                egui::RichText::new("Refreshing...")
                    .size(11.0)
                    .color(egui::Color32::from_rgb(100, 200, 255))
                    .italics(),
            );
        } else if let Some(secs) = last_poll_age {
            ui.label(
                egui::RichText::new(format!("Updated {}s ago", secs))
                    .size(11.0)
                    .color(egui::Color32::from_gray(140))
                    .italics(),
            );
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if !is_refreshing
                && ui
                    .add(
                        egui::Button::new(egui::RichText::new("⟳ Refresh").size(11.0))
                            .fill(egui::Color32::from_rgba_unmultiplied(40, 60, 90, 200)),
                    )
                    .clicked()
            {
                tc.last_list_poll = None;
            }
            if let Some(err) = &tc.last_poll_error {
                ui.label(
                    egui::RichText::new(format!("Failed: {}", err))
                        .size(11.0)
                        .color(egui::Color32::from_rgb(230, 80, 80)),
                );
            }
        });
    });
    ui.add_space(4.0);
}

#[cfg(feature = "solana")]
fn render_filter_chips(ui: &mut egui::Ui, ctx: &mut MainMenuUIContext) {
    let current = ctx
        .tournament_client
        .as_ref()
        .and_then(|tc| tc.status_filter.clone());
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        for (label, value) in [
            ("All", None),
            ("Open", Some("registration")),
            ("Active", Some("active")),
            ("Completed", Some("completed")),
        ] {
            let chip = value.map(str::to_string);
            let selected = current == chip;
            let clicked =
                crate::ui::styles::StyledButton::chip(ui, label, selected, egui::vec2(58.0, 24.0))
                    .clicked();
            if clicked {
                if let Some(tc) = ctx.tournament_client.as_deref_mut() {
                    tc.status_filter = chip;
                }
            }
        }
    });
    ui.add_space(8.0);
}

/// The player's own tournament: waiting for the next round, or the final
/// result once eliminated or champion (with a way back to the lobby).
#[cfg(feature = "solana")]
fn render_my_progress(ui: &mut egui::Ui, tc: &mut TournamentClientState) {
    if tc.active_tournament_id.is_some() && tc.waiting_for_next_match {
        ui.group(|ui| {
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("Tournament Match Complete")
                        .size(16.0)
                        .color(egui::Color32::WHITE)
                        .strong(),
                );
                ui.add_space(6.0);
                if let Some(result) = &tc.last_match_result {
                    ui.label(
                        egui::RichText::new(format!("Result: {}", result))
                            .size(14.0)
                            .color(egui::Color32::GOLD)
                            .strong(),
                    );
                }
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new("Waiting for next opponent…")
                        .size(13.0)
                        .color(egui::Color32::from_rgb(100, 200, 255))
                        .italics(),
                );
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(
                        "The next round will begin automatically when all matches finish.",
                    )
                    .size(11.0)
                    .color(egui::Color32::GRAY),
                );
            });
        });
        ui.add_space(12.0);
    }

    let terminal = matches!(
        tc.my_state,
        Some(PlayerState::Eliminated) | Some(PlayerState::Champion)
    );
    if tc.active_tournament_id.is_none() || !terminal {
        return;
    }
    let is_champion = matches!(tc.my_state, Some(PlayerState::Champion));
    let mut back_to_lobby = false;
    ui.group(|ui| {
        ui.vertical_centered(|ui| {
            let (title, colour) = if is_champion {
                ("Tournament Won", egui::Color32::GOLD)
            } else {
                ("Eliminated", egui::Color32::from_rgb(220, 120, 120))
            };
            ui.label(egui::RichText::new(title).size(16.0).color(colour).strong());
            ui.add_space(6.0);
            if let Some(place) = tc.placing {
                ui.label(
                    egui::RichText::new(format!("Final position: #{place}"))
                        .size(14.0)
                        .color(egui::Color32::WHITE),
                );
            }
            if let Some(lamports) = tc.prize_lamports.filter(|l| *l > 0) {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(format!("Prize: {:.4} SOL", lamports as f64 / 1e9))
                        .size(14.0)
                        .color(egui::Color32::GOLD)
                        .strong(),
                );
                ui.label(
                    egui::RichText::new("Paid out automatically — no claim needed.")
                        .size(11.0)
                        .color(egui::Color32::GRAY),
                );
            }
            ui.add_space(8.0);
            back_to_lobby = ui
                .button(egui::RichText::new("Back to lobby").size(12.0))
                .clicked();
        });
    });
    ui.add_space(12.0);

    if back_to_lobby {
        tc.active_tournament_id = None;
        tc.my_state = None;
        tc.placing = None;
        tc.prize_lamports = None;
        tc.last_match_result = None;
        tc.waiting_for_next_match = false;
        tc.active_game_id = None;
        tc.status_message.clear();
    }
}

#[cfg(feature = "solana")]
fn render_tournament_card(
    ui: &mut egui::Ui,
    ctx: &mut MainMenuUIContext,
    t: &TournamentSummary,
    wallet: Option<&str>,
    replay_request: &mut Option<ReplayRequest>,
) {
    let (is_password_prompt, is_expanded, is_waiting) = ctx
        .tournament_client
        .as_ref()
        .map(|tc| {
            let mine = tc.active_tournament_id == Some(t.tournament_id);
            (
                mine && t.is_private,
                tc.expanded_tournament_id == Some(t.tournament_id),
                mine && tc.is_registered(),
            )
        })
        .unwrap_or_default();

    ui.group(|ui| {
        ui.horizontal(|ui| {
            render_card_summary(ui, t);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(tc) = ctx.tournament_client.as_deref_mut() {
                    render_card_actions(ui, tc, t, wallet, is_expanded, is_password_prompt);
                }
            });
        });
        if is_expanded {
            render_card_details(ui, ctx, t, wallet, is_password_prompt, replay_request);
        }
        if is_waiting {
            let player = ctx.player_identity.display_name().to_string();
            if let Some(tc) = ctx.tournament_client.as_deref_mut() {
                render_waiting_room(ui, tc, t, wallet, &player);
            }
        }
        if is_password_prompt {
            if let Some(tc) = ctx.tournament_client.as_deref_mut() {
                render_password_prompt(ui, tc, t, wallet);
            }
        }
    });
}

/// Name, kind and status badges, entry fee and prize, registrations.
#[cfg(feature = "solana")]
fn render_card_summary(ui: &mut egui::Ui, t: &TournamentSummary) {
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(&t.name)
                    .size(14.0)
                    .color(egui::Color32::WHITE)
                    .strong(),
            );
            let (badge_text, badge_color) = if !t.is_tournament {
                ("Game", egui::Color32::from_rgb(100, 200, 180))
            } else if t.format.eq_ignore_ascii_case("single_elimination") {
                ("Elimination", egui::Color32::from_rgb(220, 130, 60))
            } else {
                ("Swiss", egui::Color32::from_rgb(255, 200, 50))
            };
            ui.add_space(4.0);
            ui.add(
                egui::Button::new(
                    egui::RichText::new(badge_text)
                        .size(9.0)
                        .color(egui::Color32::BLACK)
                        .strong(),
                )
                .fill(badge_color)
                .corner_radius(8.0)
                .min_size(egui::vec2(0.0, 16.0))
                .sense(egui::Sense::hover()),
            );
            let (status_label, status_col) = match t.status.to_lowercase().as_str() {
                s if s.contains("registration") => {
                    ("● Registration", egui::Color32::from_rgb(60, 200, 100))
                }
                s if s.contains("active") => ("● Active", egui::Color32::from_rgb(255, 180, 0)),
                s if s.contains("completed") => ("● Completed", egui::Color32::from_gray(140)),
                s if s.contains("cancelled") => {
                    ("✕ Cancelled", egui::Color32::from_rgb(220, 80, 80))
                }
                _ => ("● Unknown", egui::Color32::GRAY),
            };
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(status_label)
                    .size(10.0)
                    .color(status_col)
                    .strong(),
            );
        });
        ui.horizontal(|ui| {
            let entry = if t.entry_fee_lamports == 0 {
                "Free".to_string()
            } else {
                format!("{:.3} SOL entry", t.entry_fee_lamports as f64 / 1e9)
            };
            ui.label(
                egui::RichText::new(entry)
                    .size(11.0)
                    .color(egui::Color32::from_rgb(150, 200, 150)),
            );
            if t.prize_pool > 0 {
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(format!("Prize: {}", prize_text(t)))
                        .size(11.0)
                        .color(egui::Color32::GOLD),
                );
            }
        });
        let registered_text = if t.is_private {
            format!("Private · {}  registered", t.registered)
        } else {
            format!("{}  registered", t.registered)
        };
        ui.label(
            egui::RichText::new(registered_text)
                .size(10.0)
                .color(egui::Color32::from_gray(160)),
        );
    });
}

/// The prize pool in its currency (USDC tournaments carry a mint).
#[cfg(feature = "solana")]
fn prize_text(t: &TournamentSummary) -> String {
    if t.usdc_mint.is_some() {
        format!("{:.2} USDC", t.prize_pool as f64 / 1_000_000.0)
    } else {
        format!("{:.3} SOL", t.prize_pool as f64 / 1e9)
    }
}

/// Join, expand/collapse, and (for cancelled tournaments) remove from the list.
#[cfg(feature = "solana")]
fn render_card_actions(
    ui: &mut egui::Ui,
    tc: &mut TournamentClientState,
    t: &TournamentSummary,
    wallet: Option<&str>,
    is_expanded: bool,
    is_password_prompt: bool,
) {
    let can_join = wallet.is_some();
    let join_btn = ui.add_enabled(
        can_join && !is_password_prompt,
        egui::Button::new(
            egui::RichText::new("Join")
                .size(12.0)
                .color(egui::Color32::WHITE)
                .strong(),
        )
        .fill(if can_join {
            egui::Color32::from_rgb(50, 140, 50)
        } else {
            egui::Color32::from_rgb(60, 60, 60)
        })
        .corner_radius(4.0)
        .min_size(egui::vec2(60.0, 28.0)),
    );
    // ASCII arrows: the bundled fonts lack the geometric glyphs.
    let expand_icon = if is_expanded { "^" } else { "v" };
    if ui
        .add(
            egui::Button::new(
                egui::RichText::new(expand_icon)
                    .size(11.0)
                    .color(egui::Color32::from_gray(180)),
            )
            .fill(egui::Color32::TRANSPARENT)
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(70)))
            .min_size(egui::vec2(24.0, 26.0)),
        )
        .clicked()
    {
        tc.expanded_tournament_id = if is_expanded {
            None
        } else {
            Some(t.tournament_id)
        };
    }
    // Cancelled tournaments can't be joined or scheduled, so let the player
    // clear them out of the browser.
    if t.status.to_lowercase().contains("cancelled") {
        ui.add_space(4.0);
        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new("Remove")
                        .size(10.5)
                        .color(egui::Color32::from_gray(180)),
                )
                .fill(egui::Color32::TRANSPARENT)
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(70)))
                .min_size(egui::vec2(0.0, 26.0)),
            )
            .on_hover_text("Remove this cancelled tournament from the list")
            .clicked()
        {
            tc.dismissed_ids.insert(t.tournament_id);
        }
    }
    if join_btn.clicked() {
        join_clicked(tc, t, wallet);
    }
}

/// Join or Register: private tournaments ask for the password first; public
/// ones start registering straight away.
#[cfg(feature = "solana")]
fn join_clicked(tc: &mut TournamentClientState, t: &TournamentSummary, wallet: Option<&str>) {
    tc.active_tournament_id = Some(t.tournament_id);
    if t.is_private {
        tc.password_input.clear();
        tc.password_error = None;
        return;
    }
    match wallet {
        Some(wallet) => {
            let _ = begin_registration(tc, t.tournament_id, wallet, None);
        }
        None => warn!("[TOURNAMENT] Cannot join: wallet not connected"),
    }
}

/// Starts on-chain registration; the result arrives on `tc.tx_rx`. A bad wallet
/// key is reported on the client state and returned.
#[cfg(feature = "solana")]
fn begin_registration(
    tc: &mut TournamentClientState,
    tid: u64,
    wallet: &str,
    password: Option<String>,
) -> Result<(), String> {
    let private = password.is_some();
    match wallet.parse::<solana_sdk::pubkey::Pubkey>() {
        Ok(wallet) => {
            tc.tx_rx = Some(
                crate::multiplayer::solana::tournament::spawn_register_tournament(
                    tid, wallet, password,
                ),
            );
            tc.join_status = TournamentJoinStatus::Pending;
            tc.status_message = if private {
                "Registering on-chain and confirming private entry..."
            } else {
                "Registering on-chain and confirming with backend..."
            }
            .to_string();
            Ok(())
        }
        Err(e) => {
            let msg = format!("Bad wallet pubkey: {e}");
            warn!("[TOURNAMENT] {msg}");
            tc.join_status = TournamentJoinStatus::Error(msg.clone());
            tc.status_message = msg.clone();
            Err(msg)
        }
    }
}

/// The expanded card: format and limits, the round timer, the games list, and
/// registration while it's open.
#[cfg(feature = "solana")]
fn render_card_details(
    ui: &mut egui::Ui,
    ctx: &mut MainMenuUIContext,
    t: &TournamentSummary,
    wallet: Option<&str>,
    is_password_prompt: bool,
    replay_request: &mut Option<ReplayRequest>,
) {
    ui.separator();
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 16.0;
        if t.prize_pool > 0 {
            ui.label(
                egui::RichText::new(format!("Prize pool: {}", prize_text(t)))
                    .size(11.0)
                    .color(egui::Color32::GOLD),
            );
        }
        let kind = if !t.is_tournament {
            if t.entry_fee_lamports > 0 {
                "Wagered 1v1"
            } else {
                "Casual 1v1"
            }
        } else if t.format.eq_ignore_ascii_case("single_elimination") {
            "Single Elimination Tournament"
        } else {
            "Swiss Tournament"
        };
        ui.label(
            egui::RichText::new(format!("Format: {}", kind))
                .size(11.0)
                .color(egui::Color32::from_gray(190)),
        );
        let status_text = match t.status.to_lowercase().as_str() {
            s if s.contains("registration") => "  Open for registration",
            s if s.contains("active") => "  In progress",
            s if s.contains("completed") => "  Finished",
            s if s.contains("cancelled") => "  Cancelled",
            _ => "  Unknown status",
        };
        ui.label(
            egui::RichText::new(status_text)
                .size(11.0)
                .color(egui::Color32::from_rgb(100, 200, 255)),
        );
        if t.max_players > 0 {
            ui.label(
                egui::RichText::new(format!("{}/{} players", t.registered, t.max_players))
                    .size(11.0)
                    .color(egui::Color32::from_gray(160)),
            );
        }
    });

    // Zero and u32::MAX both mean no upper ELO cap.
    let has_max_elo = t.max_elo > 0 && t.max_elo != u32::MAX;
    if t.min_elo > 0 || has_max_elo {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.label(
                egui::RichText::new("ELO:")
                    .size(11.0)
                    .color(egui::Color32::from_gray(140)),
            );
            let range = if t.min_elo > 0 && has_max_elo {
                format!("{} – {}", t.min_elo, t.max_elo)
            } else if t.min_elo > 0 {
                format!("{}+", t.min_elo)
            } else {
                format!("≤ {}", t.max_elo)
            };
            ui.label(
                egui::RichText::new(range)
                    .size(11.0)
                    .color(egui::Color32::from_rgb(200, 180, 100)),
            );
        });
    }

    if t.status.to_lowercase().contains("active") {
        render_round_timer(ui, t);
    }
    render_games_list(ui, ctx, t, replay_request);
    if t.status.to_lowercase().contains("registration") {
        render_register_button(ui, ctx, t, wallet, is_password_prompt);
    }
    ui.add_space(4.0);
}

/// Time left in the current round, amber under five minutes, red under one.
#[cfg(feature = "solana")]
fn render_round_timer(ui: &mut egui::Ui, t: &TournamentSummary) {
    let Some(deadline) = t.round_deadline_at else {
        return;
    };
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    if deadline <= now_secs {
        return;
    }
    let remaining = (deadline - now_secs) as u64;
    let chip_color = if remaining < 60 {
        egui::Color32::from_rgb(220, 60, 60)
    } else if remaining < 300 {
        egui::Color32::from_rgb(220, 160, 40)
    } else {
        egui::Color32::from_gray(180)
    };
    ui.label(
        egui::RichText::new(format!(
            "Round ends in {}m {:02}s",
            remaining / 60,
            remaining % 60
        ))
        .size(11.0)
        .color(chip_color),
    );
}

/// Live / Upcoming / Finished tabs over the expanded tournament's games, with
/// Watch for running games and Replay for finished ones. (The client only polls
/// games for the expanded card.)
#[cfg(feature = "solana")]
fn render_games_list(
    ui: &mut egui::Ui,
    ctx: &mut MainMenuUIContext,
    t: &TournamentSummary,
    replay_request: &mut Option<ReplayRequest>,
) {
    let (games, tab) = ctx
        .tournament_client
        .as_ref()
        .map(|tc| (tc.detail_games.clone(), tc.detail_tab))
        .unwrap_or_default();
    let count = |state: TournamentGameState| games.iter().filter(|g| g.state == state).count();

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        for (label, state, this) in [
            ("Live", TournamentGameState::Live, GameTab::Live),
            ("Upcoming", TournamentGameState::Upcoming, GameTab::Upcoming),
            ("Finished", TournamentGameState::Finished, GameTab::Finished),
        ] {
            let fill = if tab == this {
                egui::Color32::from_rgb(60, 100, 160)
            } else {
                egui::Color32::from_rgb(45, 45, 55)
            };
            let clicked = ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(format!("{label} ({})", count(state)))
                            .size(11.0)
                            .color(egui::Color32::WHITE)
                            .strong(),
                    )
                    .fill(fill)
                    .corner_radius(4.0)
                    .min_size(egui::vec2(88.0, 22.0)),
                )
                .clicked();
            if clicked {
                if let Some(tc) = ctx.tournament_client.as_deref_mut() {
                    tc.detail_tab = this;
                }
            }
        }
    });

    let want = match tab {
        GameTab::Live => TournamentGameState::Live,
        GameTab::Upcoming => TournamentGameState::Upcoming,
        GameTab::Finished => TournamentGameState::Finished,
    };
    let shown: Vec<_> = games.iter().filter(|g| g.state == want).collect();
    let playlist: Vec<crate::multiplayer::spectator::SpectatorPlaylistEntry> = games
        .iter()
        .filter(|g| g.watchable)
        .map(|g| crate::multiplayer::spectator::SpectatorPlaylistEntry {
            game_id: g.game_id.to_string(),
            white: g.white_label(),
            black: g.black_label(),
            round: g.round,
        })
        .collect();

    ui.add_space(4.0);
    if shown.is_empty() {
        let empty = match tab {
            GameTab::Live => "No games in progress right now.",
            GameTab::Upcoming => "No games scheduled yet.",
            GameTab::Finished => "No games finished yet.",
        };
        ui.label(
            egui::RichText::new(empty)
                .size(11.0)
                .color(egui::Color32::from_gray(140))
                .italics(),
        );
    }

    for game in shown {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!(
                    "R{} B{}  {} vs {}",
                    game.round + 1,
                    game.match_index + 1,
                    game.white_label(),
                    game.black_label(),
                ))
                .size(11.0)
                .color(egui::Color32::from_gray(200)),
            );
            // Result for finished games, liveness for running ones.
            if let Some(r) = &game.result {
                ui.label(
                    egui::RichText::new(r)
                        .size(11.0)
                        .color(egui::Color32::from_rgb(244, 187, 68))
                        .strong(),
                );
            } else if let Some(label) = game.last_move_label() {
                ui.label(
                    egui::RichText::new(label)
                        .size(10.0)
                        .color(egui::Color32::from_gray(150)),
                );
            }
            if game.move_count > 0 {
                ui.label(
                    egui::RichText::new(format!("{} moves", game.move_count))
                        .size(10.0)
                        .color(egui::Color32::from_gray(130)),
                );
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if game.state == TournamentGameState::Finished {
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new("Replay")
                                    .size(11.0)
                                    .color(egui::Color32::WHITE)
                                    .strong(),
                            )
                            .fill(egui::Color32::from_rgb(70, 110, 80))
                            .corner_radius(5.0)
                            .min_size(egui::vec2(80.0, 24.0)),
                        )
                        .clicked()
                    {
                        *replay_request =
                            Some((game.game_id, game.white_label(), game.black_label()));
                    }
                    return;
                }
                // The backend decides watchability: an active account doesn't mean
                // any moves have been made.
                let btn = ui.add_enabled(
                    game.watchable,
                    egui::Button::new(
                        egui::RichText::new("Watch")
                            .size(11.0)
                            .color(egui::Color32::WHITE)
                            .strong(),
                    )
                    .fill(if game.watchable {
                        egui::Color32::from_rgb(60, 100, 160)
                    } else {
                        egui::Color32::from_rgb(50, 50, 60)
                    })
                    .corner_radius(5.0)
                    .min_size(egui::vec2(80.0, 24.0)),
                );
                // Say why it's disabled rather than leaving a dead grey button.
                let btn = match game.not_watchable_reason.as_deref() {
                    Some(reason) if !game.watchable => btn.on_disabled_hover_text(reason),
                    _ => btn,
                };
                if btn.clicked() {
                    if let Some(w) = ctx.spectate_events.as_mut() {
                        w.write(crate::multiplayer::spectator::SpectateViaLinkEvent {
                            game_id: game.game_id.to_string(),
                            details: Some(crate::multiplayer::spectator::SpectatorMatchDetails {
                                tournament_name: Some(t.name.clone()),
                                round: Some(game.round),
                                white: Some(game.white_label()),
                                black: Some(game.black_label()),
                            }),
                            tournament_id: Some(t.tournament_id),
                            playlist: playlist.clone(),
                        });
                    }
                }
            });
        });
        if !game.watchable && game.state == TournamentGameState::Live {
            if let Some(reason) = game.not_watchable_reason.as_deref() {
                ui.label(
                    egui::RichText::new(format!("   {reason}"))
                        .size(10.0)
                        .color(egui::Color32::from_gray(130))
                        .italics(),
                );
            }
        }
    }
}

/// Register, enabled once the wallet, rating band and balance allow it, with
/// the reason when they don't.
#[cfg(feature = "solana")]
fn render_register_button(
    ui: &mut egui::Ui,
    ctx: &mut MainMenuUIContext,
    t: &TournamentSummary,
    wallet: Option<&str>,
    is_password_prompt: bool,
) {
    ui.add_space(6.0);
    let my_elo = ctx.solana_state.as_ref().map(|s| s.cached_elo).unwrap_or(0);
    let balance = ctx.solana_state.as_ref().map(|s| s.balance).unwrap_or(0.0);
    let entry_sol = t.entry_fee_lamports as f64 / 1e9;
    let my_elo_u32 = my_elo as u32;
    let elo_ok = (t.min_elo == 0 || my_elo_u32 >= t.min_elo)
        && (t.max_elo == 0 || my_elo_u32 <= t.max_elo || my_elo == 0);
    let can_afford = t.entry_fee_lamports == 0 || balance >= entry_sol + 0.01;
    let has_wallet = wallet.is_some();
    let can_register = has_wallet && elo_ok && can_afford && !is_password_prompt;
    let tooltip = if !has_wallet {
        Some("Connect wallet to register")
    } else if !can_afford {
        Some("Insufficient balance")
    } else if !elo_ok {
        Some("ELO does not meet requirements")
    } else {
        None
    };

    ui.horizontal(|ui| {
        let btn = ui.add_enabled(
            can_register,
            egui::Button::new(
                egui::RichText::new("  Register")
                    .size(13.0)
                    .color(egui::Color32::WHITE)
                    .strong(),
            )
            .fill(if can_register {
                egui::Color32::from_rgb(60, 150, 60)
            } else {
                egui::Color32::from_rgb(60, 60, 60)
            })
            .corner_radius(5.0)
            .min_size(egui::vec2(120.0, 30.0)),
        );
        let btn = match tooltip {
            Some(tip) => btn.on_hover_text(tip),
            None => btn,
        };
        if btn.clicked() {
            if let Some(tc) = ctx.tournament_client.as_deref_mut() {
                join_clicked(tc, t, wallet);
            }
        }
        if t.entry_fee_lamports > 0 && has_wallet {
            let bal_col = if can_afford {
                egui::Color32::from_gray(130)
            } else {
                egui::Color32::from_rgb(220, 80, 80)
            };
            ui.label(
                egui::RichText::new(format!("Cost: {:.3} SOL  (bal: {:.3})", entry_sol, balance))
                    .size(10.0)
                    .color(bal_col),
            );
        }
    });
}

/// After registering: how full the field is, who's in, a chat, and Leave.
#[cfg(feature = "solana")]
fn render_waiting_room(
    ui: &mut egui::Ui,
    tc: &mut TournamentClientState,
    t: &TournamentSummary,
    wallet: Option<&str>,
    player_name: &str,
) {
    ui.add_space(6.0);
    ui.separator();
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new("WAITING ROOM")
            .size(11.0)
            .strong()
            .color(egui::Color32::from_rgb(100, 200, 255)),
    );
    ui.add_space(4.0);

    if t.max_players > 0 {
        let joined = tc.registered_players.len();
        let fill = joined.min(t.max_players) as f32 / t.max_players as f32;
        ui.add(
            egui::widgets::ProgressBar::new(fill)
                .text(format!("{}/{} players", joined, t.max_players)),
        );
        ui.add_space(4.0);
    }
    for idx in 0..t.max_players.min(8) {
        let (label, col) = match tc.registered_players.get(idx) {
            Some(name) => (name.as_str(), egui::Color32::from_rgb(200, 220, 200)),
            None => ("—", egui::Color32::from_gray(90)),
        };
        ui.label(
            egui::RichText::new(format!("{}. {}", idx + 1, label))
                .size(10.0)
                .color(col),
        );
    }
    ui.add_space(6.0);

    // Waiting-room chat.
    tc.drain_chat();
    if tc.chat_rx.is_none() {
        tc.start_chat(t.tournament_id, player_name.to_string());
    }
    ui.separator();
    ui.label(
        egui::RichText::new("CHAT")
            .size(10.0)
            .strong()
            .color(egui::Color32::from_rgb(180, 180, 220)),
    );
    egui::ScrollArea::vertical()
        .id_salt("tournament_chat_scroll")
        .max_height(80.0)
        .stick_to_bottom(true)
        .show(ui, |ui| {
            for (sender, text) in &tc.chat_messages {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        egui::RichText::new(format!("{}:", sender))
                            .size(10.0)
                            .color(egui::Color32::from_rgb(150, 200, 255))
                            .strong(),
                    );
                    ui.label(
                        egui::RichText::new(text)
                            .size(10.0)
                            .color(egui::Color32::WHITE),
                    );
                });
            }
        });
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut tc.chat_input)
                .desired_width(100.0)
                .hint_text("Message…")
                .font(egui::TextStyle::Small),
        );
        let can_send = !tc.chat_input.trim().is_empty();
        if ui
            .add_enabled(
                can_send,
                egui::Button::new(egui::RichText::new("Send").size(10.0))
                    .fill(egui::Color32::from_rgb(40, 100, 60)),
            )
            .clicked()
        {
            let text = tc.chat_input.trim().to_string();
            tc.chat_messages
                .push((player_name.to_string(), text.clone()));
            if let Some(tx) = &tc.chat_tx {
                let _ = tx.send((player_name.to_string(), text));
            }
            tc.chat_input.clear();
        }
    });
    ui.add_space(4.0);

    if ui
        .add(
            egui::Button::new(
                egui::RichText::new("Leave")
                    .size(11.0)
                    .color(egui::Color32::WHITE),
            )
            .fill(egui::Color32::from_rgb(100, 40, 40)),
        )
        .clicked()
    {
        match wallet.filter(|w| !w.is_empty()) {
            None => {
                let msg = "Connect your wallet before leaving this tournament.".to_string();
                tc.join_status = TournamentJoinStatus::Error(msg.clone());
                tc.status_message = msg;
            }
            Some(player) => {
                tc.leave_rx = Some(spawn_leave_tournament(t.tournament_id, player.to_string()));
                tc.join_status = TournamentJoinStatus::Pending;
                tc.money_flow = crate::multiplayer::solana::money_flow::MoneyFlowState::signing(
                    "leave_tournament",
                );
                tc.status_message = "Building leave transaction...".to_string();
            }
        }
    }
}

/// Leaves a tournament with a refund: the backend builds the transaction, the
/// wallet signs and sends it, the money action is recorded, and the backend
/// confirms. The signature, or the first failure, arrives on the receiver.
#[cfg(feature = "solana")]
fn spawn_leave_tournament(
    tid: u64,
    player: String,
) -> crossbeam_channel::Receiver<Result<String, String>> {
    use crate::multiplayer::network::vps::{self, ClientEvent};

    vps::emit_client_event(
        ClientEvent::new("money_action_started")
            .tournament_id(tid)
            .wallet(player.clone())
            .action("leave_tournament")
            .status("signing"),
    );
    let (tx, rx) = crossbeam_channel::bounded(1);
    crate::core::net_task::spawn_detached("leave-tournament", move || {
        let result = (|| -> Result<String, String> {
            let base = vps::vps_base();
            let client = vps::client_fast()?;
            let resp = client
                .post(format!("{base}/api/tournament/{tid}/build-leave-tx"))
                .json(&serde_json::json!({ "player": player.clone() }))
                .send()
                .map_err(|e| format!("build leave tx failed: {e}"))?;
            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().unwrap_or_default();
                return Err(format!("build leave tx failed: HTTP {status} - {body}"));
            }
            let data = resp
                .json::<serde_json::Value>()
                .map_err(|e| format!("build leave tx parse failed: {e}"))?;
            let tx_b64 = data
                .get("transaction")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "build leave tx response missing transaction".to_string())?;
            let sig = crate::multiplayer::solana::tauri_signer::sign_and_send_b64_via_tauri(
                &crate::multiplayer::solana::integration::state::DEVNET_RPC_URL,
                tx_b64,
                "Leaving tournament",
            )
            .map_err(|e| format!("wallet signing failed: {e}"))?;
            let sig_text = sig.to_string();
            vps::emit_client_event(
                ClientEvent::new("tx_submitted")
                    .tournament_id(tid)
                    .wallet(player.clone())
                    .action("leave_tournament")
                    .signature(sig_text.clone())
                    .status("submitted"),
            );
            if let Err(e) = vps::register_money_action(vps::RegisterMoneyActionReq {
                action_type: "leave_tournament".to_string(),
                scope_type: "tournament".to_string(),
                game_id: None,
                tournament_id: Some(tid as i64),
                wallet: Some(player.clone()),
                signature: Some(sig_text.clone()),
                reason: Some("player requested tournament leave/refund".to_string()),
            }) {
                tracing::warn!("[money-actions] leave_tournament register failed: {e}");
            }

            let resp = client
                .post(format!("{base}/api/tournament/{tid}/leave"))
                .json(&serde_json::json!({
                    "player": player.clone(),
                    "signature": sig_text.clone(),
                }))
                .send()
                .map_err(|e| format!("confirm leave failed: {e}"))?;
            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().unwrap_or_default();
                return Err(format!("confirm leave failed: HTTP {status} - {body}"));
            }
            Ok(sig_text)
        })();
        let _ = tx.send(result);
    });
    rx
}

/// The password field shown after joining a private tournament.
#[cfg(feature = "solana")]
fn render_password_prompt(
    ui: &mut egui::Ui,
    tc: &mut TournamentClientState,
    t: &TournamentSummary,
    wallet: Option<&str>,
) {
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Password:")
                .size(12.0)
                .color(egui::Color32::WHITE),
        );
        ui.add(
            egui::TextEdit::singleline(&mut tc.password_input)
                .password(true)
                .desired_width(120.0),
        );
        if ui
            .button(
                egui::RichText::new("Submit")
                    .size(12.0)
                    .color(egui::Color32::WHITE)
                    .strong(),
            )
            .clicked()
        {
            let password = tc.password_input.clone();
            tc.password_error = begin_registration(
                tc,
                t.tournament_id,
                wallet.unwrap_or_default(),
                Some(password),
            )
            .err();
        }
        if ui
            .button(
                egui::RichText::new("Cancel")
                    .size(12.0)
                    .color(egui::Color32::WHITE),
            )
            .clicked()
        {
            tc.active_tournament_id = None;
            tc.password_input.clear();
            tc.password_error = None;
        }
    });
    if let Some(err) = &tc.password_error {
        ui.label(
            egui::RichText::new(err)
                .size(11.0)
                .color(egui::Color32::from_rgb(255, 100, 100)),
        );
    }
}

/// Joining a private tournament by invite code.
#[cfg(feature = "solana")]
fn render_private_code_join(ui: &mut egui::Ui, tc: &mut TournamentClientState) {
    ui.add_space(12.0);
    ui.separator();
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Join with code:")
                .size(12.0)
                .color(egui::Color32::from_gray(180)),
        );
        ui.add(
            egui::TextEdit::singleline(&mut tc.private_code_input)
                .hint_text("Invite code")
                .desired_width(120.0),
        );
        let can_submit = !tc.private_code_input.is_empty();
        if ui
            .add_enabled(
                can_submit,
                egui::Button::new(egui::RichText::new("Join").size(12.0))
                    .fill(egui::Color32::from_rgb(60, 100, 160)),
            )
            .clicked()
        {
            let url = format!(
                "{}/api/tournament/join-private/{}",
                crate::multiplayer::network::vps::vps_base(),
                tc.private_code_input
            );
            let (tx, rx) = crossbeam_channel::bounded(1);
            tc.private_join_rx = Some(rx);
            tc.private_code_error = None;
            crate::core::net_task::spawn_detached("join-private", move || {
                let result = crate::multiplayer::network::vps::client_fast().and_then(|client| {
                    client
                        .post(&url)
                        .send()
                        .map_err(|e| format!("private join request failed: {e}"))
                });
                let _ = tx.send(match result {
                    Ok(r) if r.status().is_success() => Ok(()),
                    Ok(r) => Err(format!("Error {}", r.status())),
                    Err(e) => Err(e),
                });
            });
        }
    });
    if let Some(Ok(result)) = tc.private_join_rx.as_ref().map(|rx| rx.try_recv()) {
        match result {
            Ok(()) => {
                tc.join_status = TournamentJoinStatus::Pending;
                tc.private_code_input.clear();
                tc.private_code_error = None;
            }
            Err(e) => tc.private_code_error = Some(e),
        }
        tc.private_join_rx = None;
    }
    if let Some(err) = &tc.private_code_error {
        ui.label(
            egui::RichText::new(err)
                .size(11.0)
                .color(egui::Color32::from_rgb(255, 100, 100)),
        );
    }
}

/// Fetches a finished game's PGN and returns to the menu, where the replay
/// opens once it arrives.
#[cfg(feature = "solana")]
fn start_replay(ctx: &mut MainMenuUIContext, (game_id, white, black): ReplayRequest) {
    ctx.menu_state.set(crate::core::MenuState::Main);
    let (tx, rx) = crossbeam_channel::bounded(1);
    crate::core::net_task::spawn_detached("tournament-replay", move || {
        let _ = tx.send(crate::multiplayer::network::vps::fetch_game_pgn(game_id));
    });
    ctx.pgn_replay_fetch.pending =
        Some(crate::states::main_menu::tournament_replay::PendingPgnReplay { rx, white, black });
}
