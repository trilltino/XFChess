//! In-game profile panel — overlay showing username, ELO, W/L/D, recent games, ELO sparkline.
//!
//! Toggled open by setting `ProfileViewState.open = true`.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

use crate::multiplayer::solana::addon::SolanaProfile;
use crate::multiplayer::solana::integration::state::SolanaIntegrationState;

// ── Types ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct GameHistoryEntry {
    pub game_id: String,
    pub result: String,
    pub opponent: Option<String>,
    pub timestamp: i64,
    pub stake_amount: f64,
}

#[derive(Resource, Default)]
pub struct ProfileViewState {
    pub open: bool,
    pub history: Vec<GameHistoryEntry>,
    pub fetching: bool,
    pub fetch_rx: Option<crossbeam_channel::Receiver<Result<Vec<GameHistoryEntry>, String>>>,
    /// Estimated ELO progression computed from history (newest first → reversed for chart)
    pub elo_curve: Vec<f32>,
}

impl ProfileViewState {
    /// Rebuild the ELO sparkline from history (oldest→newest, +15 win, -15 loss, +5 draw).
    pub fn rebuild_curve(&mut self, base_elo: u16) {
        let mut elo = base_elo as f32;
        let mut curve: Vec<f32> = vec![elo];
        for entry in self.history.iter().rev() {
            elo += match entry.result.as_str() {
                "win" => 15.0,
                "loss" => -15.0,
                "draw" => 5.0,
                _ => 0.0,
            };
            elo = elo.max(100.0);
            curve.push(elo);
        }
        self.elo_curve = curve;
    }
}

// ── VPS fetch helper ────────────────────────────────────────────────────────

fn fetch_history_blocking(wallet: String) -> Result<Vec<GameHistoryEntry>, String> {
    use crate::multiplayer::network::vps::{client, vps_base};
    let url = format!("{}/ratings/history/{}", vps_base(), wallet);
    let resp = client()
        .map_err(|e| e.clone())?
        .get(url)
        .send()
        .map_err(|e| format!("fetch_history: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("fetch_history: HTTP {}", resp.status()));
    }
    let v: serde_json::Value = resp.json().map_err(|e| format!("parse: {e}"))?;
    let arr = v["history"].as_array().cloned().unwrap_or_default();
    Ok(arr
        .into_iter()
        .map(|item| GameHistoryEntry {
            game_id: item["game_id"].as_str().unwrap_or("").to_string(),
            result: item["result"].as_str().unwrap_or("unknown").to_string(),
            opponent: item["opponent"].as_str().map(|s| s.to_string()),
            timestamp: item["timestamp"].as_i64().unwrap_or(0),
            stake_amount: item["stake_amount"].as_f64().unwrap_or(0.0),
        })
        .collect())
}

// ── Systems ─────────────────────────────────────────────────────────────────

/// Kick off a history fetch whenever the panel is first opened.
pub fn fetch_profile_history(
    mut view: ResMut<ProfileViewState>,
    solana_state: Res<SolanaIntegrationState>,
    mut was_open: Local<bool>,
) {
    if !view.open {
        *was_open = false;
        return;
    }
    // Only fetch once per open
    if *was_open || view.fetching || view.fetch_rx.is_some() {
        return;
    }
    let Some(pk) = solana_state.wallet_pubkey else {
        return;
    };
    let wallet = pk.to_string();
    *was_open = true;
    view.fetching = true;

    let (tx, rx) = crossbeam_channel::bounded(1);
    view.fetch_rx = Some(rx);

    std::thread::spawn(move || {
        let _ = tx.send(fetch_history_blocking(wallet));
    });
}

/// Poll the fetch channel and populate history + elo_curve.
pub fn poll_profile_history(
    mut view: ResMut<ProfileViewState>,
    solana_state: Res<SolanaIntegrationState>,
) {
    let rx = match view.fetch_rx.take() {
        Some(r) => r,
        None => return,
    };
    match rx.try_recv() {
        Ok(Ok(history)) => {
            view.history = history;
            view.rebuild_curve(solana_state.cached_elo);
            view.fetching = false;
        }
        Ok(Err(e)) => {
            warn!("[PROFILE VIEW] history fetch failed: {e}");
            view.fetching = false;
        }
        Err(crossbeam_channel::TryRecvError::Empty) => {
            view.fetch_rx = Some(rx); // put it back
        }
        Err(_) => {
            view.fetching = false;
        }
    }
}

/// Render the profile overlay panel.
pub fn profile_view_ui(
    mut contexts: EguiContexts,
    mut view: ResMut<ProfileViewState>,
    solana_state: Res<SolanaIntegrationState>,
    profile: Option<Res<SolanaProfile>>,
) {
    if !view.open {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    let mut open = view.open;
    egui::Window::new("Profile")
        .open(&mut open)
        .resizable(true)
        .default_width(480.0)
        .default_height(520.0)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .frame(
            egui::Frame::window(&ctx.style())
                .fill(egui::Color32::from_rgba_premultiplied(18, 18, 22, 250))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(230, 57, 70))),
        )
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                // ── Header ──────────────────────────────────────────────
                let username = solana_state
                    .cached_display_name
                    .as_deref()
                    .unwrap_or("Anonymous");
                let elo = if solana_state.cached_elo > 0 {
                    solana_state.cached_elo as u32
                } else {
                    profile.as_ref().map(|p| p.elo).unwrap_or(0)
                };

                ui.horizontal(|ui| {
                    ui.heading(
                        egui::RichText::new(username)
                            .color(egui::Color32::WHITE)
                            .size(24.0),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(format!("ELO {}", elo))
                            .color(egui::Color32::from_rgb(230, 57, 70))
                            .size(18.0)
                            .strong(),
                    );
                });

                if let Some(pk) = solana_state.wallet_pubkey {
                    let s = pk.to_string();
                    ui.label(
                        egui::RichText::new(format!("{}…{}", &s[..6], &s[s.len() - 4..]))
                            .color(egui::Color32::GRAY)
                            .monospace()
                            .size(11.0),
                    );
                }

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(8.0);

                // ── Stats row ───────────────────────────────────────────
                let (wins, losses, draws, games) = if let Some(ref p) = profile {
                    (p.wins, p.losses, p.draws, p.games_played())
                } else {
                    (0, 0, 0, 0)
                };

                ui.horizontal(|ui| {
                    stat_card(ui, "W", wins, egui::Color32::from_rgb(34, 197, 94));
                    ui.add_space(8.0);
                    stat_card(ui, "L", losses, egui::Color32::from_rgb(239, 68, 68));
                    ui.add_space(8.0);
                    stat_card(ui, "D", draws, egui::Color32::from_rgb(156, 163, 175));
                    ui.add_space(8.0);
                    stat_card(ui, "Total", games, egui::Color32::WHITE);
                });

                ui.add_space(16.0);

                // ── ELO sparkline ───────────────────────────────────────
                if view.elo_curve.len() >= 2 {
                    ui.label(
                        egui::RichText::new("ELO Progression")
                            .color(egui::Color32::GRAY)
                            .size(12.0),
                    );
                    ui.add_space(4.0);
                    render_sparkline(ui, &view.elo_curve, egui::vec2(ui.available_width(), 60.0));
                    ui.add_space(12.0);
                } else if view.fetching {
                    ui.label(
                        egui::RichText::new("Loading history…")
                            .color(egui::Color32::GRAY)
                            .size(12.0),
                    );
                    ui.add_space(12.0);
                }

                ui.separator();
                ui.add_space(8.0);

                // ── Recent games ────────────────────────────────────────
                ui.label(egui::RichText::new("Recent Games").size(13.0).strong());
                ui.add_space(6.0);

                if view.history.is_empty() && !view.fetching {
                    ui.label(
                        egui::RichText::new("No games recorded yet.")
                            .color(egui::Color32::GRAY)
                            .size(12.0),
                    );
                }

                egui::ScrollArea::vertical()
                    .max_height(200.0)
                    .show(ui, |ui| {
                        for entry in view.history.iter().take(20) {
                            let (result_text, result_color) = match entry.result.as_str() {
                                "win" => ("WIN", egui::Color32::from_rgb(34, 197, 94)),
                                "loss" => ("LOSS", egui::Color32::from_rgb(239, 68, 68)),
                                "draw" => ("DRAW", egui::Color32::from_rgb(156, 163, 175)),
                                _ => ("?", egui::Color32::GRAY),
                            };
                            ui.horizontal(|ui| {
                                ui.colored_label(
                                    result_color,
                                    egui::RichText::new(result_text)
                                        .monospace()
                                        .size(11.0)
                                        .strong(),
                                );
                                ui.add_space(6.0);
                                let opp = entry.opponent.as_deref().unwrap_or("Unknown");
                                let short_opp = if opp.len() > 12 {
                                    format!("{}…{}", &opp[..6], &opp[opp.len() - 4..])
                                } else {
                                    opp.to_string()
                                };
                                ui.label(
                                    egui::RichText::new(format!("vs {}", short_opp))
                                        .color(egui::Color32::LIGHT_GRAY)
                                        .size(11.0),
                                );
                                if entry.stake_amount > 0.0 {
                                    ui.add_space(4.0);
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "{:.3} SOL",
                                            entry.stake_amount
                                        ))
                                        .color(egui::Color32::from_rgb(230, 57, 70))
                                        .size(10.0),
                                    );
                                }
                            });
                        }
                    });
            });
        });

    view.open = open;
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn stat_card(ui: &mut egui::Ui, label: &str, value: u32, color: egui::Color32) {
    ui.group(|ui| {
        ui.set_min_width(60.0);
        ui.vertical_centered(|ui| {
            ui.label(
                egui::RichText::new(value.to_string())
                    .color(color)
                    .size(20.0)
                    .strong(),
            );
            ui.label(
                egui::RichText::new(label)
                    .color(egui::Color32::GRAY)
                    .size(11.0),
            );
        });
    });
}

fn render_sparkline(ui: &mut egui::Ui, curve: &[f32], size: egui::Vec2) {
    let (response, painter) = ui.allocate_painter(size, egui::Sense::hover());
    let rect = response.rect;

    let min_v = curve.iter().cloned().fold(f32::INFINITY, f32::min);
    let max_v = curve.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let range = (max_v - min_v).max(1.0);

    let n = curve.len();
    let points: Vec<egui::Pos2> = curve
        .iter()
        .enumerate()
        .map(|(i, &v)| {
            let x = rect.left() + (i as f32 / (n - 1) as f32) * rect.width();
            let y = rect.bottom() - ((v - min_v) / range) * rect.height();
            egui::pos2(x, y)
        })
        .collect();

    // Fill area under curve
    if points.len() >= 2 {
        let mut fill_pts = points.clone();
        fill_pts.push(egui::pos2(rect.right(), rect.bottom()));
        fill_pts.push(egui::pos2(rect.left(), rect.bottom()));
        painter.add(egui::Shape::convex_polygon(
            fill_pts,
            egui::Color32::from_rgba_premultiplied(230, 57, 70, 30),
            egui::Stroke::NONE,
        ));
    }

    // Line
    for w in points.windows(2) {
        painter.line_segment(
            [w[0], w[1]],
            egui::Stroke::new(2.0, egui::Color32::from_rgb(230, 57, 70)),
        );
    }

    // Current ELO dot
    if let Some(&last) = points.last() {
        painter.circle_filled(last, 4.0, egui::Color32::WHITE);
    }
}
