//! Game Over popup — result, ELO delta, wager settlement, post-game actions.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

use crate::core::GameState;
use crate::ui::styles::*;
use crate::game::components::piece_types::PieceColor;
use crate::game::resources::{GameOverState, MoveHistory};

// ── Payout resource ───────────────────────────────────────────────────────────

/// Payout and meta information populated when the game ends.
#[derive(Resource, Debug, Clone, Default)]
pub struct GameOverPayoutInfo {
    // Wager
    pub wager_amount: u64,
    pub country_fee: u64,
    pub elo_fee: u64,
    pub rent_return: u64,
    pub winning_prize: u64,
    pub is_draw: bool,

    // Identity
    pub player_color: Option<PieceColor>,
    pub local_player_pubkey: Option<String>,
    pub game_id: u64,

    // Settlement
    pub payout_confirmed: bool,
    pub finalize_sig: Option<String>,
    pub game_ended_at: Option<std::time::Instant>,
    pub settlement_started_at: Option<std::time::Instant>,
    pub dispute_pending: bool,
    pub dispute_sig: Option<String>,

    // ELO / rating
    /// true = Rated or Wager match; false = free/practice (hide ELO section)
    pub is_rated: bool,
    pub elo_before: u32,
    pub elo_after: u32,
    pub win_streak: u32,

    // Tournament
    /// Set when this game was part of a tournament.
    pub tournament_id: Option<u64>,
    /// USDC mint address when prize is paid in USDC (otherwise SOL).
    pub usdc_mint: Option<String>,
    /// USDC prize amount in token base units (6 decimals).
    pub usdc_prize_usdc: Option<u64>,
}

impl GameOverPayoutInfo {
    fn format_sol(lamports: u64) -> String {
        format!("{:.3} SOL", lamports as f64 / 1_000_000_000.0)
    }

    pub fn is_wager_game(&self) -> bool {
        self.wager_amount > 0
    }

    pub fn player_winnings(&self) -> u64 {
        if self.is_draw {
            self.wager_amount.saturating_sub(self.country_fee / 2).saturating_sub(self.elo_fee / 2)
        } else {
            self.winning_prize
        }
    }

    pub fn dispute_window_open(&self) -> bool {
        match self.game_ended_at {
            Some(t) => t.elapsed().as_secs() < 48 * 3600,
            None => false,
        }
    }

    /// Seconds since on-chain settlement was initiated.
    pub fn settlement_elapsed(&self) -> u64 {
        self.settlement_started_at.map(|t| t.elapsed().as_secs()).unwrap_or(0)
    }

    pub fn elo_delta(&self) -> i32 {
        if self.elo_before == 0 || self.elo_after == 0 { return 0; }
        self.elo_after as i32 - self.elo_before as i32
    }
}

// ── Popup animation ───────────────────────────────────────────────────────────

/// Controls fade-in animation on popup entry.
#[derive(Resource, Default)]
pub struct PopupAnimState {
    pub elapsed: f32,
}

impl PopupAnimState {
    /// 0.0 → 1.0 over 200 ms.
    pub fn t(&self) -> f32 {
        (self.elapsed / 0.2).min(1.0)
    }
}

fn setup_popup_anim(mut commands: Commands) {
    commands.insert_resource(PopupAnimState::default());
}

// ── PGN cache ────────────────────────────────────────────────────────────────

/// PGN and final FEN computed once when the game ends (OnEnter GameOver).
/// Avoids replaying all moves on every button click in the game-over popup.
#[derive(Resource, Default)]
pub struct CachedGamePgn {
    pub pgn: Option<nimzovich_engine::ParsedPgnGame>,
    pub pgn_string: String,
    pub final_fen: String,
    /// Set to `true` once the authoritative Braid/VPS PGN has replaced the local one.
    /// While `false`, Review/Save buttons show a subtle loading indicator.
    pub braid_pgn_ready: bool,
}

pub fn cache_pgn_on_game_over(
    history: Res<MoveHistory>,
    game_over: Res<GameOverState>,
    mut cached: ResMut<CachedGamePgn>,
) {
    let pgn_result = match game_over.winner() {
        Some(PieceColor::White) => "1-0",
        Some(PieceColor::Black) => "0-1",
        None                    => "1/2-1/2",
    };
    let pgn = build_pgn(&history, pgn_result);
    cached.pgn_string = pgn_to_string(&pgn);
    cached.final_fen = build_final_fen(&history);
    cached.pgn = Some(pgn);
    cached.braid_pgn_ready = false; // Braid VPS fetch will set this true when it arrives
}

// ── PGN helpers ───────────────────────────────────────────────────────────────

/// Replay all moves and return the final position as a FEN string.
fn build_final_fen(history: &MoveHistory) -> String {
    use nimzovich_engine::{do_move_with_promo, game_to_fen, new_game};
    use crate::game::components::PieceType;

    let mut game = new_game();
    for rec in &history.moves {
        let src = rec.from.1 as i8 * 8 + rec.from.0 as i8;
        let dst = rec.to.1 as i8 * 8 + rec.to.0 as i8;
        let is_promo = rec.piece_type == PieceType::Pawn && (rec.to.1 == 7 || rec.to.1 == 0);
        let promo: i8 = if is_promo { 5 } else { 0 };
        do_move_with_promo(&mut game, src, dst, true, promo);
    }
    game_to_fen(&game)
}

/// Return the name of the ELO tier that an ELO value falls into.
fn elo_tier(elo: u32) -> Option<&'static str> {
    match elo {
        0..=1199    => None,
        1200..=1399 => Some("Beginner"),
        1400..=1599 => Some("Intermediate"),
        1600..=1799 => Some("Advanced"),
        1800..=1999 => Some("Expert"),
        2000..=2199 => Some("Master"),
        2200..=2399 => Some("International Master"),
        _           => Some("Grandmaster"),
    }
}

/// Convert the game's MoveHistory to a `ParsedPgnGame` using the engine
/// to derive proper SAN notation.  Promotions default to Queen because
/// MoveRecord doesn't store the promoted-to piece.
fn build_pgn(history: &MoveHistory, result_str: &str) -> nimzovich_engine::ParsedPgnGame {
    use nimzovich_engine::{do_move_with_promo, move_to_san, new_game};
    use crate::game::components::PieceType;
    use std::collections::BTreeMap;

    let mut game = new_game();
    let mut san_moves: Vec<String> = Vec::with_capacity(history.moves.len());

    for rec in &history.moves {
        let src = rec.from.1 as i8 * 8 + rec.from.0 as i8;
        let dst = rec.to.1 as i8 * 8 + rec.to.0 as i8;
        // Default promo to queen (5); MoveRecord doesn't store promoted piece type
        let is_promo = rec.piece_type == PieceType::Pawn && (rec.to.1 == 7 || rec.to.1 == 0);
        let promo: i8 = if is_promo { 5 } else { 0 };

        let san = move_to_san(&game, src, dst, promo);
        san_moves.push(san);
        do_move_with_promo(&mut game, src, dst, true, promo);
    }

    let mut tags = BTreeMap::new();
    tags.insert("Event".to_string(), "XFChess Game".to_string());
    tags.insert("Site".to_string(), "xfchess.app".to_string());
    tags.insert("Date".to_string(), chrono_or_unknown());
    tags.insert("Result".to_string(), result_str.to_string());

    nimzovich_engine::ParsedPgnGame { tags, moves: san_moves, result: result_str.to_string(), per_ply_annotations: Vec::new() }
}

fn chrono_or_unknown() -> String {
    // No chrono dep — use a placeholder date.
    "????.??.??".to_string()
}

/// Render a PGN string from a `ParsedPgnGame`.
pub fn pgn_to_string(pgn: &nimzovich_engine::ParsedPgnGame) -> String {
    let mut out = String::new();
    for (k, v) in &pgn.tags {
        out.push_str(&format!("[{} \"{}\"]\n", k, v));
    }
    out.push('\n');

    let mut line = String::new();
    let mut ply = 0usize;
    let mut flush = |line: &mut String, out: &mut String| {
        if !line.is_empty() {
            out.push_str(line.trim_end());
            out.push('\n');
            line.clear();
        }
    };
    for mv in &pgn.moves {
        let token = if ply % 2 == 0 {
            format!("{}. {}", ply / 2 + 1, mv)
        } else {
            mv.clone()
        };
        if !line.is_empty() && line.len() + 1 + token.len() > 80 {
            flush(&mut line, &mut out);
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(&token);
        ply += 1;
    }
    if !line.is_empty() {
        if line.len() + 1 + pgn.result.len() > 80 {
            out.push_str(line.trim_end());
            out.push('\n');
            out.push_str(&pgn.result);
        } else {
            out.push_str(line.trim_end());
            out.push(' ');
            out.push_str(&pgn.result);
        }
    } else {
        out.push_str(&pgn.result);
    }
    out.push('\n');
    out
}

// ── Main popup system ─────────────────────────────────────────────────────────

pub fn game_over_popup_system(
    mut contexts: EguiContexts,
    game_over: Res<GameOverState>,
    payout_info: Option<Res<GameOverPayoutInfo>>,
    cached_pgn: Res<CachedGamePgn>,
    mut next_state: ResMut<NextState<GameState>>,
    mut game_mode: ResMut<crate::core::GameMode>,
    mut anim: ResMut<PopupAnimState>,
    time: Res<Time>,
    mut commands: Commands,
    tokio_runtime: Option<Res<crate::multiplayer::TokioRuntime>>,
) {
    anim.elapsed += time.delta_secs();
    let t = anim.t();
    let alpha = (220.0 * t) as u8;

    let Ok(ctx) = contexts.ctx_mut() else { return };

    // ── colour palette ────────────────────────────────────────────────────────
    let text_primary  = egui::Color32::from_rgba_unmultiplied(240, 240, 240, alpha);
    let text_secondary= egui::Color32::from_rgba_unmultiplied(160, 160, 160, alpha);
    let text_gold     = egui::Color32::from_rgba_unmultiplied(244, 187, 68, alpha);
    let text_green    = egui::Color32::from_rgba_unmultiplied(80, 220, 120, alpha);
    let text_red      = egui::Color32::from_rgba_unmultiplied(255, 100, 100, alpha);
    let text_blue     = egui::Color32::from_rgba_unmultiplied(100, 180, 255, alpha);

    let frame = StyledPanel::popup_alpha(alpha);

    // ── POV headline ──────────────────────────────────────────────────────────
    let player_color = payout_info.as_ref().and_then(|p| p.player_color);
    let (headline, headline_color) = match (game_over.winner(), player_color) {
        (Some(w), Some(pc)) if w == pc => ("You Won", text_gold),
        (Some(_), Some(_))             => ("You Lost", text_red),
        (None, _)                      => ("Draw", text_secondary),
        (Some(PieceColor::White), None)=> ("White Wins", text_primary),
        (Some(PieceColor::Black), None)=> ("Black Wins", text_secondary),
    };

    let termination = game_over.termination_text();

    let mut trigger_dispute  = false;
    let mut trigger_review   = false;
    let mut trigger_analyze  = false;
    let mut save_pgn         = false;
    let mut play_again_bot   = false;
    let mut go_to_bracket    = false;
    let mut trigger_rematch  = false;

    let is_single_player = matches!(*game_mode, crate::core::GameMode::SinglePlayer);
    let _is_online = matches!(*game_mode,
        crate::core::GameMode::BraidMultiplayer | crate::core::GameMode::MultiplayerCompetitive
    );

    egui::Window::new("")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size([380.0, 340.0])
        .frame(frame)
        .show(ctx, |ui| {
            ui.set_width(348.0);
            ui.vertical_centered(|ui| {

                // ── Result ───────────────────────────────────────────────────
                ui.label(TextStyle::popup_title("GAME OVER").color(egui::Color32::from_rgba_unmultiplied(255, 255, 255, alpha)));
                ui.add_space(4.0);
                ui.label(egui::RichText::new(headline)
                    .size(22.0)
                    .family(egui::FontFamily::Name("CinzelBold".into()))
                    .color(headline_color));
                if !termination.is_empty() {
                    ui.label(egui::RichText::new(termination)
                        .size(13.0).italics().color(text_secondary));
                }


                // ── ELO / rating ─────────────────────────────────────────────
                if let Some(info) = payout_info.as_ref() {
                    if !info.is_rated {
                        ui.add_space(6.0);
                        ui.label(egui::RichText::new("Practice game — no rating change")
                            .size(11.0).color(text_secondary));
                    } else {
                        let delta = info.elo_delta();
                        if info.elo_before > 0 {
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                let delta_text = if delta >= 0 {
                                    format!("+{}", delta)
                                } else {
                                    format!("{}", delta)
                                };
                                let delta_color = if delta >= 0 { text_green } else { text_red };
                                ui.label(egui::RichText::new(
                                    format!("Rating: {} → {}", info.elo_before, info.elo_after)
                                ).size(13.0).color(text_primary));
                                ui.label(egui::RichText::new(delta_text)
                                    .size(13.0).strong().color(delta_color));
                                if info.win_streak >= 3 {
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        ui.label(egui::RichText::new(
                                            format!("{}-game streak!", info.win_streak)
                                        ).size(11.0).color(text_gold));
                                    });
                                }
                            });
                            // Tier-up badge
                            let old_tier = elo_tier(info.elo_before);
                            let new_tier = elo_tier(info.elo_after);
                            if new_tier != old_tier {
                                if let Some(tier) = new_tier {
                                    ui.add_space(4.0);
                                    ui.horizontal_centered(|ui| {
                                        ui.label(egui::RichText::new(format!("Tier Up!  {}", tier))
                                            .size(14.0).strong().color(text_gold));
                                    });
                                }
                            }
                        }
                    }
                }

                // ── Wager settlement ─────────────────────────────────────────
                if let Some(info) = payout_info.as_ref() {
                    if info.is_wager_game() {
                        ui.add_space(10.0);
                        ui.separator();
                        ui.add_space(6.0);
                        ui.label(egui::RichText::new("Payout Details")
                            .size(13.0).strong().color(text_gold));
                        ui.add_space(6.0);

                        // Fee rows
                        let rows: &[(&str, i64, bool)] = &[
                            ("Pot (2× wager)", (info.wager_amount * 2) as i64, true),
                            ("Treasury fee",   -(info.country_fee as i64), false),
                            ("ELO fee",        -(info.elo_fee as i64), false),
                            ("Rent returned",  info.rent_return as i64, true),
                        ];
                        for (label, lamports, is_positive) in rows {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(*label)
                                    .size(11.0).color(text_secondary));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    let sign = if *lamports < 0 { "- " } else { "+ " };
                                    let color = if *is_positive { text_green } else { text_red };
                                    let sol = GameOverPayoutInfo::format_sol((*lamports).unsigned_abs());
                                    ui.label(egui::RichText::new(format!("{}{}", sign, sol))
                                        .size(11.0).color(color));
                                });
                            });
                        }

                        ui.add_space(4.0);
                        ui.separator();
                        ui.add_space(4.0);

                        let prize_label = if info.is_draw { "Returned:" } else { "You Won:" };
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(prize_label)
                                .size(14.0).strong().color(text_gold));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                // USDC prize path
                                if let (Some(_mint), Some(usdc)) = (&info.usdc_mint, info.usdc_prize_usdc) {
                                    let usdc_fmt = format!("{:.2} USDC", usdc as f64 / 1_000_000.0);
                                    ui.label(egui::RichText::new(usdc_fmt)
                                        .size(16.0).strong().color(text_gold));
                                } else {
                                    ui.label(egui::RichText::new(
                                        GameOverPayoutInfo::format_sol(info.player_winnings())
                                    ).size(16.0).strong().color(text_gold));
                                }
                            });
                        });
                        if info.is_draw {
                            if let (Some(_mint), Some(usdc)) = (&info.usdc_mint, info.usdc_prize_usdc) {
                                let usdc_fmt = format!("{:.2} USDC", usdc as f64 / 1_000_000.0);
                                ui.label(egui::RichText::new(
                                    format!("Both players receive: {}", usdc_fmt)
                                ).size(11.0).italics().color(text_secondary));
                            } else {
                                ui.label(egui::RichText::new(
                                    format!("Both players receive: {}", GameOverPayoutInfo::format_sol(info.player_winnings()))
                                ).size(11.0).italics().color(text_secondary));
                            }
                        }

                        ui.add_space(4.0);
                        // Settlement status row
                        if info.payout_confirmed {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("Prize claimed  ✓")
                                    .size(12.0).color(text_green));
                                if let Some(sig) = &info.finalize_sig {
                                    let short = format!("{}...", &sig[..8.min(sig.len())]);
                                    let url = format!("https://solscan.io/tx/{}?cluster=devnet", sig);
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.link(egui::RichText::new(short)
                                            .size(11.0).color(text_secondary)).clicked()
                                        {
                                            open_url(&url);
                                        }
                                    });
                                }
                            });
                        } else {
                            let elapsed = info.settlement_elapsed();
                            if elapsed < 60 {
                                let dots = match elapsed % 3 { 0 => ".", 1 => "..", _ => "..." };
                                ui.label(egui::RichText::new(
                                    format!("Settling on-chain{} ({}s)", dots, elapsed)
                                ).size(11.0).italics().color(text_secondary));
                            } else {
                                let url = info.finalize_sig.as_deref()
                                    .map(|s| format!("https://solscan.io/tx/{}?cluster=devnet", s))
                                    .unwrap_or_default();
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new("Taking longer than expected — ")
                                        .size(11.0).color(text_red));
                                    if !url.is_empty() && ui.link(
                                        egui::RichText::new("check Solscan").size(11.0).color(text_blue)
                                    ).clicked() {
                                        open_url(&url);
                                    }
                                });
                            }
                        }

                        // Dispute button — loser only, within 48 h
                        let is_loser = matches!(
                            (game_over.winner(), player_color),
                            (Some(PieceColor::White), Some(PieceColor::Black))
                            | (Some(PieceColor::Black), Some(PieceColor::White))
                        );
                        if is_loser && info.payout_confirmed && info.dispute_window_open() {
                            ui.add_space(4.0);
                            if let Some(sig) = &info.dispute_sig {
                                ui.label(egui::RichText::new(
                                    format!("Dispute submitted ({}...)", &sig[..8.min(sig.len())])
                                ).size(11.0).color(text_secondary));
                            } else if ui.button(
                                egui::RichText::new("Dispute Result").size(12.0)
                            ).clicked() {
                                trigger_dispute = true;
                            }
                        }
                    }
                }

                ui.add_space(6.0);
                ui.add(egui::Separator::default().shrink(40.0));
                ui.add_space(4.0);

                // ── Secondary actions ─────────────────────────────────────────
                {
                    let has_rematch = payout_info.as_ref().map(|i| i.game_id > 0).unwrap_or(false);
                    let dark_btn = egui::Color32::from_rgba_unmultiplied(40, 40, 44, 200);
                    let spacing = 6.0_f32;
                    let btn_w = 80.0_f32;
                    let n = if has_rematch { 4 } else { 3 };
                    let total = n as f32 * btn_w + (n - 1) as f32 * spacing;
                    let pad = ((ui.available_width() - total) / 2.0).max(0.0);
                    ui.horizontal(|ui| {
                        ui.add_space(pad);
                        ui.spacing_mut().item_spacing.x = spacing;
                        let pgn_loading = !cached_pgn.braid_pgn_ready && cached_pgn.pgn.is_some();
                        let review_label = if pgn_loading { "Review ⟳" } else { "Review" };
                        let save_label   = if pgn_loading { "Save ⟳" }   else { "Save PGN" };
                        let dim = if pgn_loading {
                            egui::Color32::from_rgba_unmultiplied(40, 40, 44, 120)
                        } else {
                            dark_btn
                        };
                        let resp_review = ui.add_sized([btn_w, 26.0], egui::Button::new(
                            egui::RichText::new(review_label).size(11.0)
                        ).fill(dim));
                        if pgn_loading { resp_review.on_hover_text("Fetching authoritative game record…"); }
                        else if resp_review.clicked() { trigger_review = true; }

                        let resp_analyze = ui.add_sized([btn_w, 26.0], egui::Button::new(
                            egui::RichText::new("Analyze").size(11.0)
                        ).fill(dark_btn));
                        if resp_analyze.clicked() { trigger_analyze = true; }

                        let resp_save = ui.add_sized([btn_w, 26.0], egui::Button::new(
                            egui::RichText::new(save_label).size(11.0)
                        ).fill(dim));
                        if pgn_loading { resp_save.on_hover_text("Fetching authoritative game record…"); }
                        else if resp_save.clicked() { save_pgn = true; }
                        if has_rematch {
                            if ui.add_sized([btn_w, 26.0], egui::Button::new(
                                egui::RichText::new("Rematch").size(11.0)
                            ).fill(dark_btn)).clicked() {
                                trigger_rematch = true;
                            }
                        }
                    });
                }

                ui.add_space(8.0);

                // ── Primary actions ───────────────────────────────────────────
                {
                    let spacing = 8.0_f32;
                    let btn_w = 130.0_f32;
                    let total = 2.0 * btn_w + spacing;
                    let pad = ((ui.available_width() - total) / 2.0).max(0.0);
                    ui.horizontal(|ui| {
                        ui.add_space(pad);
                        ui.spacing_mut().item_spacing.x = spacing;
                        let play_label = if is_single_player { "Play Again" } else { "New Game" };
                        if ui.add_sized([btn_w, 32.0], egui::Button::new(
                            egui::RichText::new(play_label).size(12.0).strong()
                                .color(egui::Color32::from_rgb(20, 18, 10))
                        ).fill(egui::Color32::from_rgba_unmultiplied(244, 187, 68, 220))).clicked() {
                            if is_single_player { play_again_bot = true; } else { next_state.set(GameState::InGame); }
                        }
                        let back_label = if payout_info.as_ref().and_then(|p| p.tournament_id).is_some() {
                            "Back to Bracket"
                        } else {
                            "Main Menu"
                        };
                        if ui.add_sized([btn_w, 32.0], egui::Button::new(
                            egui::RichText::new(back_label).size(12.0)
                        ).fill(egui::Color32::from_rgba_unmultiplied(40, 40, 44, 200))).clicked() {
                            if payout_info.as_ref().and_then(|p| p.tournament_id).is_some() {
                                go_to_bracket = true;
                            } else {
                                next_state.set(GameState::MainMenu);
                            }
                        }
                    });
                }
            });
        });

    // ── deferred actions ──────────────────────────────────────────────────────
    if trigger_dispute {
        commands.insert_resource(PendingDispute { game_id: 0, sig_rx: None });
    }

    if trigger_review {
        if let Some(pgn) = cached_pgn.pgn.clone() {
            *game_mode = crate::core::GameMode::PgnReplay;
            commands.insert_resource(crate::game::replay::ParsedPgnGameResource { inner: pgn, show_eval_graph: false, puzzle_mode: false, puzzle_revealed: false });
            next_state.set(GameState::InGame);
        }
    }

    if trigger_analyze {
        if let Some(pgn) = cached_pgn.pgn.clone() {
            *game_mode = crate::core::GameMode::PgnReplay;
            commands.insert_resource(crate::game::replay::ParsedPgnGameResource { inner: pgn, show_eval_graph: true, puzzle_mode: false, puzzle_revealed: false });
            next_state.set(GameState::InGame);
        }
    }

    if save_pgn {
        let pgn_text = cached_pgn.pgn_string.clone();
        std::thread::spawn(move || {
            let base = dirs::document_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."));
            let dir = base.join("xfchess");
            let _ = std::fs::create_dir_all(&dir);
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let path = dir.join(format!("game_{}.pgn", timestamp));
            let _ = std::fs::write(&path, pgn_text);
        });
    }

    if play_again_bot {
        *game_mode = crate::core::GameMode::SinglePlayer;
        next_state.set(GameState::InGame);
    }

    if go_to_bracket {
        next_state.set(GameState::MainMenu);
    }

    if trigger_rematch {
        if let (Some(info), Some(rt)) = (payout_info.as_ref(), tokio_runtime.as_ref()) {
            let game_id = info.game_id;
            let base = crate::multiplayer::network::vps::vps_base();
            rt.0.spawn(async move {
                let url = format!("{}/api/games/rematch/{}", base, game_id);
                let _ = reqwest::Client::new().post(&url).send().await;
            });
        }
    }
}

// ── Spectator end-of-game overlay ────────────────────────────────────────────

/// Read-only game-over popup shown when the local player is spectating.
/// No payout, ELO, or dispute rows — just result + "Review Game" button.
pub fn spectator_game_over_overlay(
    mut contexts: EguiContexts,
    game_over: Res<GameOverState>,
    cached_pgn: Res<CachedGamePgn>,
    mut next_state: ResMut<NextState<GameState>>,
    mut game_mode: ResMut<crate::core::GameMode>,
    mut commands: Commands,
    mut anim: ResMut<PopupAnimState>,
    time: Res<Time>,
) {
    if !game_over.is_game_over() { return; }

    anim.elapsed += time.delta_secs();
    let t = anim.t();
    let alpha = (220.0 * t) as u8;

    let Ok(ctx) = contexts.ctx_mut() else { return };

    let text_p  = egui::Color32::from_rgba_unmultiplied(240, 240, 240, alpha);
    let text_s  = egui::Color32::from_rgba_unmultiplied(160, 160, 160, alpha);
    let text_g  = egui::Color32::from_rgba_unmultiplied(244, 187, 68, alpha);

    let frame = StyledPanel::popup_alpha(alpha);

    let headline = match game_over.winner() {
        Some(PieceColor::White) => ("White Wins!", text_p),
        Some(PieceColor::Black) => ("Black Wins!", text_s),
        None                    => ("Draw",        text_s),
    };
    let termination = game_over.termination_text();
    let pgn_result = match game_over.winner() {
        Some(PieceColor::White) => "1-0",
        Some(PieceColor::Black) => "0-1",
        None                    => "1/2-1/2",
    };

    let mut trigger_review = false;

    egui::Window::new("spectator_game_over")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size([340.0, 200.0])
        .frame(frame)
        .show(ctx, |ui| {
            ui.set_width(300.0);
            ui.vertical_centered(|ui| {
                ui.label(TextStyle::popup_body("SPECTATING").color(egui::Color32::from_rgba_unmultiplied(155, 158, 175, alpha)));
                ui.add_space(4.0);
                ui.label(TextStyle::popup_title("GAME OVER").color(egui::Color32::from_rgba_unmultiplied(255, 255, 255, alpha)));
                ui.add_space(4.0);
                ui.label(egui::RichText::new(headline.0).size(22.0)
                    .family(egui::FontFamily::Name("CinzelBold".into()))
                    .color(headline.1));
                if !termination.is_empty() {
                    ui.label(egui::RichText::new(termination).size(12.0).italics().color(text_s));
                }
                ui.add_space(16.0);
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    if ui.add_sized([130.0, 38.0], egui::Button::new(
                        egui::RichText::new("Review Game").size(13.0).strong()
                    ).fill(egui::Color32::from_rgb(30, 60, 100))).clicked() {
                        trigger_review = true;
                    }
                    if ui.add_sized([100.0, 38.0], egui::Button::new(
                        egui::RichText::new("Leave").size(13.0)
                    ).fill(egui::Color32::from_rgb(60, 30, 30))).clicked() {
                        next_state.set(GameState::MainMenu);
                    }
                });
                ui.add_space(6.0);
                ui.label(egui::RichText::new(format!("Result: {}", pgn_result))
                    .size(10.0).color(text_g));
            });
        });

    if trigger_review {
        if let Some(pgn) = cached_pgn.pgn.clone() {
            *game_mode = crate::core::GameMode::PgnReplay;
            commands.insert_resource(crate::game::replay::ParsedPgnGameResource { inner: pgn, show_eval_graph: false, puzzle_mode: false, puzzle_revealed: false });
            next_state.set(GameState::InGame);
        }
    }
}

// ── Dispute system (unchanged logic) ─────────────────────────────────────────

/// Resource inserted when the player clicks "Dispute Result".
#[derive(Resource)]
pub struct PendingDispute {
    pub game_id: u64,
    pub sig_rx: Option<crossbeam_channel::Receiver<String>>,
}

pub fn apply_dispute_trigger(
    mut commands: Commands,
    mut dispute: Option<ResMut<PendingDispute>>,
    #[cfg(feature = "solana")]
    rollup: Option<Res<crate::multiplayer::rollup::manager::EphemeralRollupManager>>,
    mut payout_info: Option<ResMut<GameOverPayoutInfo>>,
) {
    let Some(ref mut dispute) = dispute else { return };

    if let Some(ref rx) = dispute.sig_rx {
        if let Ok(sig) = rx.try_recv() {
            if let Some(ref mut info) = payout_info {
                info.dispute_sig = Some(sig);
                info.dispute_pending = false;
            }
            commands.remove_resource::<PendingDispute>();
            return;
        }
        return;
    }

    #[cfg(feature = "solana")]
    let game_id = if let Some(mgr) = rollup {
        if mgr.game_id != 0 { mgr.game_id } else { dispute.game_id }
    } else {
        dispute.game_id
    };
    #[cfg(not(feature = "solana"))]
    let game_id = dispute.game_id;

    let Some(ref mut info) = payout_info else {
        commands.remove_resource::<PendingDispute>();
        return;
    };
    if info.dispute_pending { return; }
    info.dispute_pending = true;
    let player = info.local_player_pubkey.clone().unwrap_or_default();

    let (sig_tx, sig_rx) = crossbeam_channel::bounded::<String>(1);
    dispute.sig_rx = Some(sig_rx);

    std::thread::spawn(move || {
        use crate::multiplayer::vps_client;
        match vps_client::vps_submit_dispute(game_id, &player) {
            Ok(sig)  => { let _ = sig_tx.send(sig); }
            Err(e)   => error!("[DISPUTE] Failed for game {}: {e}", game_id),
        }
    });
}

// ── On-chain payout fetch ─────────────────────────────────────────────────────

#[cfg(feature = "solana")]
pub fn fetch_game_payout_info(
    game_over: Res<GameOverState>,
    solana_sync: Option<Res<crate::multiplayer::solana::addon::SolanaGameSync>>,
    competitive: Option<Res<crate::multiplayer::solana::addon::CompetitiveMatchState>>,
    mut payout_info: ResMut<GameOverPayoutInfo>,
    current_turn: Option<Res<crate::game::resources::CurrentTurn>>,
) {
    if !game_over.is_game_over() { return; }

    *payout_info = GameOverPayoutInfo::default();

    if let Some(sync) = solana_sync {
        if sync.wager_amount > 0 {
            payout_info.wager_amount = sync.wager_amount;
            payout_info.is_rated = true;

            if let Some(_comp) = competitive {
                payout_info.country_fee = 10_000_000;
                payout_info.elo_fee = 10_000_000;
                payout_info.rent_return = 2_280_000;
                let total_pot = sync.wager_amount * 2;
                payout_info.winning_prize = total_pot
                    .saturating_sub(payout_info.country_fee)
                    .saturating_sub(payout_info.elo_fee);
            }

            payout_info.is_draw = game_over.winner().is_none();
            payout_info.settlement_started_at = Some(std::time::Instant::now());
            payout_info.game_ended_at = Some(std::time::Instant::now());

            if let Some(turn) = current_turn {
                payout_info.player_color = Some(turn.color);
            }
        }
    }
}

// ── URL open helper ───────────────────────────────────────────────────────────

fn open_url(url: &str) {
    let url = url.to_string();
    std::thread::spawn(move || {
        #[cfg(target_os = "windows")]
        let _ = std::process::Command::new("cmd").args(["/c", "start", "", &url]).spawn();
        #[cfg(target_os = "macos")]
        let _ = std::process::Command::new("open").arg(&url).spawn();
        #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
        let _ = std::process::Command::new("xdg-open").arg(&url).spawn();
    });
}

// ── Plugin ────────────────────────────────────────────────────────────────────

pub struct GameOverPopupPlugin;

impl Plugin for GameOverPopupPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameOverPayoutInfo>();
        app.init_resource::<PopupAnimState>();
        app.init_resource::<CachedGamePgn>();

        app.add_systems(OnEnter(GameState::GameOver), (setup_popup_anim, cache_pgn_on_game_over));

        app.add_systems(
            bevy_egui::EguiPrimaryContextPass,
            game_over_popup_system
                .run_if(in_state(GameState::GameOver))
                .run_if(|gm: Res<crate::core::GameMode>| {
                    *gm != crate::core::GameMode::Spectator
                }),
        );

        app.add_systems(
            bevy_egui::EguiPrimaryContextPass,
            spectator_game_over_overlay
                .run_if(in_state(GameState::GameOver))
                .run_if(|gm: Res<crate::core::GameMode>| {
                    *gm == crate::core::GameMode::Spectator
                }),
        );

        app.add_systems(Update, apply_dispute_trigger);

        #[cfg(feature = "solana")]
        app.add_systems(OnEnter(GameState::GameOver), fetch_game_payout_info);
    }
}
