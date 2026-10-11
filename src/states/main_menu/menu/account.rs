use super::*;

pub(super) fn render_solana_connect_panel(ui: &mut egui::Ui, cx: &mut MainMenuUIContext) {
    // Back button + header
    ui.horizontal(|ui| {
        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new("‹ Back")
                        .size(10.0)
                        .color(egui::Color32::from_rgba_unmultiplied(180, 180, 200, 160)),
                )
                .fill(egui::Color32::TRANSPARENT)
                .stroke(egui::Stroke::NONE),
            )
            .clicked()
        {
            *cx.menu_panel = MenuPanel::PlayOnline;
        }
        ui.label(
            egui::RichText::new("Solana Multiplayer")
                .size(10.0)
                .color(egui::Color32::from_rgba_unmultiplied(180, 180, 200, 160))
                .family(egui::FontFamily::Proportional)
                .strong(),
        );
    });
    let sep_rect = ui.available_rect_before_wrap();
    let sep_y = ui.cursor().top() + 3.0;
    ui.painter().hline(
        sep_rect.left()..=sep_rect.left() + W,
        sep_y,
        egui::Stroke::new(
            1.0,
            egui::Color32::from_rgba_unmultiplied(220, 220, 240, 60),
        ),
    );
    ui.add_space(10.0);

    const W: f32 = 280.0;
    const SP: f32 = 6.0;

    let wallet_connected = cx.wallet_bridge.enabled && cx.wallet_bridge.known_pubkey.is_some();

    // Connect Wallet is always the first item
    let connect_label = if wallet_connected {
        "Wallet Connected"
    } else {
        "Connect Wallet"
    };
    if ui
        .add_sized(
            [W, 40.0],
            egui::Button::new(
                egui::RichText::new(connect_label)
                    .size(11.6)
                    .color(egui::Color32::WHITE)
                    .strong()
                    .family(egui::FontFamily::Proportional),
            )
            .fill(if wallet_connected {
                egui::Color32::from_rgb(30, 110, 60)
            } else {
                egui::Color32::from_rgb(50, 120, 200)
            })
            .corner_radius(6.0)
            .stroke(egui::Stroke::new(
                1.0,
                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 40),
            )),
        )
        .clicked()
    {
        cx.wallet_bridge.status_rx = None;
        cx.wallet_bridge.balance_rx = None;
        cx.wallet_bridge.enabled = true;
        cx.wallet_bridge.timer = 5.0;
        cx.wallet_bridge.show_connect_overlay = true;
        // Use shared wallet-popup discovery so the announced instance port wins over scanning.
        #[cfg(feature = "solana")]
        crate::multiplayer::solana::tauri_signer::open_wallet_browser();
    }

    // Wagered options only shown once wallet is connected
    if wallet_connected {
        ui.add_space(SP + 6.0);

        if item(ui, "Wagered PVP", W) {
            #[cfg(feature = "solana")]
            {
                // create_game/join_game require an existing PlayerProfile; complete the
                // profile gate before submitting either transaction.
                let profile_ready = cx.solana_state.as_ref().map(|s| {
                    s.profile_status == crate::multiplayer::solana::integration::state::ProfileStatus::HasProfileWithUsername
                }).unwrap_or(false);
                if !profile_ready {
                    crate::multiplayer::solana::tauri_signer::open_profile_step();
                } else {
                    if let Some(lobby) = cx.solana_lobby.as_mut() {
                        lobby.mode = crate::multiplayer::solana::lobby::LobbyMode::Create;
                        lobby.allow_create = true;
                        // Reset stale create-game status on fresh menu entry.
                        lobby.status = crate::multiplayer::solana::lobby::LobbyStatus::Idle;
                        // Always start the wager at $0 — don't carry over whatever
                        // amount was left typed in from a previous visit.
                        lobby.wager_sol = 0.0;
                        lobby.wager_amount_input.clear();
                    }
                    cx.menu_state.set(crate::core::MenuState::SolanaLobby);
                }
            }
            #[cfg(not(feature = "solana"))]
            cx.menu_state.set(crate::core::MenuState::BraidLobby);
        }
        ui.add_space(SP);

        if item(ui, "Find Wagered Game", W) {
            #[cfg(feature = "solana")]
            {
                // Same profile gate as "Wagered PVP" above — `join_game` also
                // requires the joiner's PlayerProfile PDA to already exist.
                let profile_ready = cx.solana_state.as_ref().map(|s| {
                    s.profile_status == crate::multiplayer::solana::integration::state::ProfileStatus::HasProfileWithUsername
                }).unwrap_or(false);
                if !profile_ready {
                    crate::multiplayer::solana::tauri_signer::open_profile_step();
                } else {
                    if let Some(lobby) = cx.solana_lobby.as_mut() {
                        lobby.mode = crate::multiplayer::solana::lobby::LobbyMode::Browse;
                        // Finding a game is join/browse only — hide the Create tab.
                        lobby.allow_create = false;
                        lobby.status = crate::multiplayer::solana::lobby::LobbyStatus::Idle;
                    }
                    cx.menu_state.set(crate::core::MenuState::SolanaLobby);
                }
            }
            #[cfg(not(feature = "solana"))]
            cx.menu_state.set(crate::core::MenuState::BraidLobby);
        }
        ui.add_space(SP);

        // Reuse the on-chain tournament browser from the Solana submenu.
        if item(ui, "Tournaments", W) {
            cx.menu_state.set(MenuState::Tournaments);
        }
    } else {
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new("Connect a Solana wallet to access wagered games")
                .size(8.6)
                .color(egui::Color32::from_rgb(130, 130, 150))
                .italics(),
        );
    }
}

pub(super) fn render_profile_panel(ui: &mut egui::Ui, cx: &mut MainMenuUIContext) {
    // Header row
    ui.horizontal(|ui| {
        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new("‹ Back")
                        .size(10.0)
                        .color(egui::Color32::from_rgba_unmultiplied(180, 180, 200, 160)),
                )
                .fill(egui::Color32::TRANSPARENT)
                .stroke(egui::Stroke::NONE),
            )
            .clicked()
        {
            *cx.menu_panel = MenuPanel::Main;
        }
        ui.add_space(6.0);
        ui.label(
            egui::RichText::new("Profile")
                .size(16.5)
                .color(egui::Color32::WHITE)
                .family(egui::FontFamily::Proportional)
                .strong(),
        );
    });
    ui.add_space(16.0);

    const W: f32 = 280.0;
    const SP: f32 = 6.0;

    let connected = cx.wallet_bridge.enabled && cx.wallet_bridge.known_pubkey.is_some();

    if connected {
        // Username
        let name = cx.player_identity.display_name().to_string();
        ui.label(
            egui::RichText::new(&name)
                .size(18.0)
                .color(egui::Color32::WHITE)
                .strong()
                .family(egui::FontFamily::Proportional),
        );
        ui.add_space(4.0);

        let elo = cx.player_identity.display_elo();
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("ELO")
                    .size(9.0)
                    .color(egui::Color32::from_rgb(120, 140, 170)),
            );
            ui.label(
                egui::RichText::new(&elo)
                    .size(11.0)
                    .color(egui::Color32::from_rgb(200, 220, 255))
                    .strong(),
            );
        });
        ui.add_space(2.0);

        // Lichess ELO — a distinct, clearly-labeled second stat. Never
        // merged with the on-chain ELO above.
        if let Some(lichess_elo) = cx.player_identity.lichess_elo {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("LICHESS")
                        .size(9.0)
                        .color(egui::Color32::from_rgb(120, 140, 170)),
                );
                ui.label(
                    egui::RichText::new(lichess_elo.to_string())
                        .size(11.0)
                        .color(egui::Color32::from_rgb(200, 255, 220))
                        .strong(),
                );
                if cx.player_identity.lichess_verified {
                    ui.label(
                        egui::RichText::new("✓")
                            .size(10.0)
                            .color(egui::Color32::from_rgb(120, 220, 140)),
                    );
                }
            });
            ui.add_space(2.0);
        }

        if let Some(ref country) = cx.player_identity.country {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Country")
                        .size(9.0)
                        .color(egui::Color32::from_rgb(120, 140, 170)),
                );
                ui.label(
                    egui::RichText::new(country)
                        .size(11.0)
                        .color(egui::Color32::from_rgb(200, 210, 200)),
                );
            });
            ui.add_space(2.0);
        }

        if let Some(ref pk) = cx.wallet_bridge.known_pubkey.clone() {
            let short = format!(
                "{}...{}",
                &pk[..6.min(pk.len())],
                &pk[pk.len().saturating_sub(4)..]
            );
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Wallet")
                        .size(9.0)
                        .color(egui::Color32::from_rgb(120, 140, 170)),
                );
                ui.label(
                    egui::RichText::new(&short)
                        .size(9.5)
                        .color(egui::Color32::from_rgb(160, 180, 160))
                        .monospace(),
                );
            });
        }

        // SOL balance — always shown once connected
        {
            let data = cx.wallet_bridge.data.lock();
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Balance")
                        .size(9.0)
                        .color(egui::Color32::from_rgb(120, 140, 170)),
                );
                let sol_text = format!("{:.4} SOL", data.sol_balance);
                let sol_color = if data.sol_balance > 0.0 {
                    egui::Color32::from_rgb(140, 220, 140)
                } else {
                    egui::Color32::from_rgb(140, 140, 150)
                };
                ui.label(egui::RichText::new(&sol_text).size(11.0).color(sol_color));
                if let Some(usd) = data.usd_balance {
                    if usd > 0.0 {
                        ui.label(
                            egui::RichText::new(format!("(${:.2})", usd))
                                .size(9.0)
                                .color(egui::Color32::from_rgb(140, 160, 140)),
                        );
                    }
                }
            });
        }

        // The backend completes PKCE linking; the regular auth/me poll picks up
        // the result after opening the OAuth browser flow.
        if cx.player_identity.lichess_elo.is_none() {
            ui.add_space(SP);
            if ui
                .add_sized(
                    [W, 34.0],
                    egui::Button::new(
                        egui::RichText::new("Connect Lichess")
                            .size(11.0)
                            .color(egui::Color32::WHITE)
                            .strong()
                            .family(egui::FontFamily::Proportional),
                    )
                    .fill(egui::Color32::from_rgb(30, 90, 60))
                    .corner_radius(6.0),
                )
                .clicked()
            {
                if let Some(pubkey) = cx.wallet_bridge.known_pubkey.clone() {
                    crate::multiplayer::network::vps::start_lichess_link(pubkey);
                }
            }
        }

        ui.add_space(SP + 6.0);

        // Disconnect
        if ui
            .add_sized(
                [W, 34.0],
                egui::Button::new(
                    egui::RichText::new("Disconnect Wallet")
                        .size(10.8)
                        .color(egui::Color32::WHITE)
                        .family(egui::FontFamily::Proportional),
                )
                .fill(egui::Color32::from_rgb(100, 40, 40))
                .corner_radius(6.0),
            )
            .clicked()
        {
            #[cfg(feature = "solana")]
            complete_logout(cx.tokio_runtime.0.handle());
            cx.wallet_bridge.enabled = false;
            cx.wallet_bridge.known_pubkey = None;
            cx.wallet_bridge.timer = 0.0;
            cx.wallet_bridge.status_rx = None;
            cx.wallet_bridge.balance_rx = None;
            {
                let mut d = cx.wallet_bridge.data.lock();
                d.sol_balance = 0.0;
                d.usd_balance = None;
                d.balance_loaded = false;
            }
            #[cfg(feature = "solana")]
            if let Some(wallet) = cx.wallet.as_mut() {
                wallet.pubkey = None;
                wallet.keypair = None;
                wallet.ranked_active = false;
                wallet.tournament_match_id = None;
            }
            #[cfg(feature = "solana")]
            if let Some(sync) = cx.solana_sync.as_mut() {
                sync.game_id = None;
                sync.moves_submitted = 0;
                sync.wager_amount = 0;
            }
            // See the Settings-panel Logout handler's comment on why this
            // reset is required — same gap existed here.
            #[cfg(feature = "solana")]
            if let Some(solana_state) = cx.solana_state.as_mut() {
                **solana_state =
                    crate::multiplayer::solana::integration::state::SolanaIntegrationState::default(
                    );
            }
            #[cfg(feature = "solana")]
            if let Some(lobby) = cx.solana_lobby.as_mut() {
                **lobby = crate::multiplayer::solana::lobby::SolanaLobbyState::default();
            }
            *cx.player_identity = crate::states::main_menu::PlayerIdentity::default();
        }
        ui.add_space(SP);

        // Create Profile — shown when wallet is connected but no backend profile found
        if cx.player_identity.username.is_none() {
            ui.add_space(SP);
            ui.label(
                egui::RichText::new("No profile found for this wallet.")
                    .size(9.0)
                    .color(egui::Color32::from_rgb(200, 160, 80))
                    .italics(),
            );
            ui.add_space(4.0);
            if ui
                .add_sized(
                    [W, 34.0],
                    egui::Button::new(
                        egui::RichText::new("Create Profile")
                            .size(11.0)
                            .color(egui::Color32::WHITE)
                            .strong()
                            .family(egui::FontFamily::Proportional),
                    )
                    .fill(egui::Color32::from_rgb(130, 80, 20))
                    .corner_radius(6.0)
                    .stroke(egui::Stroke::new(
                        1.0,
                        egui::Color32::from_rgba_unmultiplied(255, 200, 80, 80),
                    )),
                )
                .clicked()
            {
                // Same override pattern as the "XFChess.com" button above:
                // XFCHESS_WEB_URL for the local dev stack, else the public site.
                let base = std::env::var("XFCHESS_WEB_URL")
                    .unwrap_or_else(|_| "https://xfchess.com".to_string());
                let _ = webbrowser::open(&format!("{base}/create-profile"));
            }
            ui.add_space(SP);
        }

        // Refresh balance
        if ui
            .add_sized(
                [W, 30.0],
                egui::Button::new(
                    egui::RichText::new("Refresh")
                        .size(10.0)
                        .color(egui::Color32::from_rgb(180, 200, 220))
                        .family(egui::FontFamily::Proportional),
                )
                .fill(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 12))
                .corner_radius(6.0),
            )
            .clicked()
        {
            // Force a status re-poll and clear balance_rx so balance is re-fetched too
            cx.wallet_bridge.timer = 5.0;
            cx.wallet_bridge.balance_rx = None;
            // Reset balance display so user sees it's refreshing
            {
                let mut d = cx.wallet_bridge.data.lock();
                d.sol_balance = 0.0;
                d.usd_balance = None;
                d.balance_loaded = false;
            }
        }
    } else {
        // Not connected — direct user to the correct place
        ui.label(
            egui::RichText::new("No wallet connected")
                .size(13.0)
                .color(egui::Color32::from_rgb(160, 160, 180))
                .italics(),
        );
        ui.add_space(SP + 4.0);
        ui.label(
            egui::RichText::new("Connect your wallet from the Solana Multiplayer menu.")
                .size(9.2)
                .color(egui::Color32::from_rgb(120, 130, 150)),
        );
        ui.add_space(SP + 4.0);

        if ui
            .add_sized(
                [W, 34.0],
                egui::Button::new(
                    egui::RichText::new("Go to Solana Multiplayer →")
                        .size(10.5)
                        .color(egui::Color32::from_rgb(140, 180, 255))
                        .family(egui::FontFamily::Proportional),
                )
                .fill(egui::Color32::from_rgba_unmultiplied(40, 80, 160, 60))
                .corner_radius(6.0)
                .stroke(egui::Stroke::new(
                    1.0,
                    egui::Color32::from_rgba_unmultiplied(80, 140, 255, 80),
                )),
            )
            .clicked()
        {
            *cx.menu_panel = MenuPanel::PlayOnline;
        }
    }
}

pub(super) const FAUCET_URL: &str = "https://faucet.solana.com";

pub(super) const COPIED_FLASH_SECS: f64 = 1.6;

/// The bottom edge of the wallet HUD as last drawn (egui temp data); absent while
/// it is hidden. The menu window starts below it on short windows.
fn wallet_hud_bottom_id() -> egui::Id {
    egui::Id::new("wallet_hud_bottom")
}

pub(super) fn wallet_hud_bottom(ctx: &egui::Context) -> Option<f32> {
    ctx.data(|d| d.get_temp::<f32>(wallet_hud_bottom_id()))
}

pub fn render_wallet_hud(ctx: &egui::Context, cx: &mut MainMenuUIContext) {
    let display_name = cx.player_identity.display_name().to_string();
    let is_guest = cx.player_identity.username.is_none();

    // PlayerIdentity exposes the bridge pubkey even without the solana feature,
    // so players can copy it before funding their wallet.
    let pubkey_str = cx
        .player_identity
        .pubkey_str
        .clone()
        .filter(|s| !s.is_empty());

    #[cfg(feature = "solana")]
    let sol_balance = cx.solana_state.as_ref().map(|s| s.balance).unwrap_or(0.0);
    #[cfg(not(feature = "solana"))]
    let sol_balance = cx.wallet_bridge.data.lock().sol_balance;

    #[cfg(feature = "solana")]
    let connected = cx
        .solana_state
        .as_ref()
        .and_then(|s| s.wallet_pubkey)
        .is_some();
    #[cfg(not(feature = "solana"))]
    let connected = false;

    if !connected && is_guest {
        ctx.data_mut(|d| d.remove::<f32>(wallet_hud_bottom_id()));
        return;
    }

    let (sol_usd_rate, sol_gbp_rate, balance_loaded) = {
        let d = cx.wallet_bridge.data.lock();
        (d.sol_usd_rate, d.sol_gbp_rate, d.balance_loaded)
    };

    // 0 = USD (default — the primary display currency throughout the app),
    // 1 = SOL, 2 = GBP — persisted in egui temp storage across frames.
    let currency_id = egui::Id::new("balance_currency");
    let currency_mode = ctx.data(|d| d.get_temp::<u8>(currency_id).unwrap_or(0));

    // Fetch is in flight (fresh connect / manual refresh) — show a loading
    // state instead of a stale/default "$0.00" that reads as a real balance.
    let (balance_text, balance_color) = if !balance_loaded {
        (
            "Updating…".to_string(),
            egui::Color32::from_rgb(160, 170, 190),
        )
    } else {
        match currency_mode {
            1 => (
                format!("{:.3} SOL", sol_balance),
                egui::Color32::from_rgb(20, 241, 149),
            ),
            2 => {
                if sol_gbp_rate > 0.0 {
                    (
                        format!("£{:.2}", sol_balance * sol_gbp_rate),
                        egui::Color32::from_rgb(20, 241, 149),
                    )
                } else {
                    (
                        format!("{:.3} SOL", sol_balance),
                        egui::Color32::from_rgb(20, 241, 149),
                    )
                }
            }
            _ => {
                if sol_usd_rate > 0.0 {
                    (
                        format!("${:.2}", sol_balance * sol_usd_rate),
                        egui::Color32::from_rgb(20, 241, 149),
                    )
                } else {
                    (
                        format!("{:.3} SOL", sol_balance),
                        egui::Color32::from_rgb(20, 241, 149),
                    )
                }
            }
        }
    };

    let hud = egui::Area::new("wallet_hud".into())
        .anchor(egui::Align2::LEFT_TOP, egui::vec2(20.0, 20.0))
        .show(ctx, |ui| {
            egui::Frame {
                corner_radius: egui::CornerRadius::same(8),
                fill: egui::Color32::from_rgba_unmultiplied(20, 20, 25, 220),
                stroke: egui::Stroke::new(
                    1.0,
                    egui::Color32::from_rgba_unmultiplied(255, 255, 255, 30),
                ),
                inner_margin: egui::Margin::symmetric(16, 12),
                ..egui::Frame::NONE
            }
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    match &pubkey_str {
                        Some(pk) => {
                            let copied_id = egui::Id::new("wallet_hud_name_copied_at");
                            let now = ctx.input(|i| i.time);
                            let copied_at = ctx.data(|d| d.get_temp::<f64>(copied_id));
                            let just_copied =
                                copied_at.is_some_and(|t| now - t < COPIED_FLASH_SECS);

                            let resp = ui.add(
                                egui::Button::new(
                                    egui::RichText::new(if just_copied {
                                        "Address copied!"
                                    } else {
                                        display_name.as_str()
                                    })
                                    .size(15.5)
                                    .color(if just_copied {
                                        egui::Color32::from_rgb(20, 241, 149)
                                    } else {
                                        egui::Color32::WHITE
                                    })
                                    .strong(),
                                )
                                .fill(egui::Color32::TRANSPARENT)
                                .stroke(egui::Stroke::NONE),
                            );

                            if resp.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }

                            let resp = resp.on_hover_text(format!(
                                "Click to copy your wallet address\n{pk}\n\n\
                                 Out of SOL? Paste it into {FAUCET_URL} for free devnet \
                                 SOL, then come back and play."
                            ));

                            if resp.clicked() {
                                ui.output_mut(|o| {
                                    o.commands.push(egui::OutputCommand::CopyText(pk.clone()))
                                });
                                ctx.data_mut(|d| d.insert_temp(copied_id, now));
                            }

                            // Request redraws so the copied-address notification expires without user input.
                            if just_copied {
                                ctx.request_repaint();
                            }
                        }
                        None => {
                            ui.label(
                                egui::RichText::new(&display_name)
                                    .size(15.5)
                                    .color(egui::Color32::WHITE)
                                    .strong(),
                            );
                        }
                    }

                    // Balance/price only makes sense with a wallet connected —
                    // a local/guest profile shows just the name, nothing else.
                    if connected {
                        ui.add_space(12.0);
                        ui.separator();
                        ui.add_space(8.0);

                        // Clickable balance — cycles SOL → USD → GBP on each click.
                        let bal_resp = ui.add(
                            egui::Button::new(
                                egui::RichText::new(&balance_text)
                                    .size(15.0)
                                    .color(balance_color)
                                    .strong(),
                            )
                            .fill(egui::Color32::TRANSPARENT)
                            .stroke(egui::Stroke::NONE),
                        );
                        if bal_resp.clicked() {
                            ctx.data_mut(|d| d.insert_temp(currency_id, (currency_mode + 1) % 3));
                        }
                        if bal_resp.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                    }
                });
            });
        });
    let bottom = hud.response.rect.bottom();
    ctx.data_mut(|d| d.insert_temp(wallet_hud_bottom_id(), bottom));
}
