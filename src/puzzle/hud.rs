//! The in-game puzzle card: what to find, progress, hint, result, and "Next".

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

use super::session::{PuzzleBoard, PuzzlePhase, PuzzleSession, StartPuzzle};
use crate::core::state::GameState;
use crate::rendering::pieces::PieceColor;
use crate::ui::styles::colors::UiColors;

const LAMPORTS_PER_SOL: f64 = 1_000_000_000.0;

pub(super) fn puzzle_hud(
    mut contexts: EguiContexts,
    mut session: ResMut<PuzzleSession>,
    board: Res<PuzzleBoard>,
    mut start: MessageWriter<StartPuzzle>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    let Ok(ctx) = contexts.ctx_mut() else { return };
    let Some(puzzle) = session.active.clone() else {
        return;
    };

    egui::Window::new("Puzzle")
        .title_bar(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_TOP, [0.0, 56.0])
        .frame(
            egui::Frame::window(&ctx.global_style())
                .fill(UiColors::BG_DARK)
                .inner_margin(egui::Margin::symmetric(16, 12)),
        )
        .show(ctx, |ui| {
            ui.set_width(300.0);
            let side = match board.player_color {
                PieceColor::White => "White",
                PieceColor::Black => "Black",
            };
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("Puzzle {}", puzzle.rating))
                        .strong()
                        .color(UiColors::ACCENT_GOLD),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(format!("{side} to play"))
                            .color(UiColors::TEXT_SECONDARY),
                    );
                });
            });
            if !puzzle.themes.is_empty() {
                ui.label(
                    egui::RichText::new(puzzle.themes.join(" · "))
                        .small()
                        .color(UiColors::TEXT_TERTIARY),
                );
            }
            ui.add_space(6.0);
            ui.label(format!(
                "Moves found: {} / {}",
                session.found, puzzle.solution_len
            ));
            if let Some(lamports) = puzzle.reward_lamports {
                ui.label(
                    egui::RichText::new(format!(
                        "Reward: {:.3} SOL",
                        lamports as f64 / LAMPORTS_PER_SOL
                    ))
                    .color(UiColors::SUCCESS),
                );
            }
            ui.label(egui::RichText::new(&session.status).color(UiColors::TEXT_PRIMARY));

            if let Some(result) = &session.result {
                ui.add_space(4.0);
                let (headline, colour) = if result.win {
                    ("Solved", UiColors::SUCCESS)
                } else {
                    ("Failed", UiColors::DANGER)
                };
                ui.label(egui::RichText::new(headline).strong().color(colour));
                if session.hinted {
                    ui.label(
                        egui::RichText::new("Unrated — a hint was used")
                            .color(UiColors::TEXT_SECONDARY),
                    );
                } else {
                    ui.label(format!(
                        "Rating {} ({:+})",
                        result.rating, result.rating_diff
                    ));
                }
                if result.paid_lamports > 0 {
                    ui.label(
                        egui::RichText::new(format!(
                            "Paid {:.3} SOL",
                            result.paid_lamports as f64 / LAMPORTS_PER_SOL
                        ))
                        .color(UiColors::SUCCESS),
                    );
                }
                if puzzle.source == "lichess" {
                    ui.label(
                        egui::RichText::new("Result sent to Lichess")
                            .small()
                            .color(UiColors::TEXT_TERTIARY),
                    );
                }
            }

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let playing = session.phase == PuzzlePhase::Playing;
                if ui
                    .add_enabled(playing, egui::Button::new("Hint"))
                    .on_hover_text("Shows which piece to move; the puzzle becomes unrated")
                    .clicked()
                {
                    session.request_hint();
                }
                let can_load = !session.is_busy()
                    && matches!(session.phase, PuzzlePhase::Playing | PuzzlePhase::Done);
                let label = if session.phase == PuzzlePhase::Done {
                    "Next"
                } else {
                    "Skip"
                };
                if ui.add_enabled(can_load, egui::Button::new(label)).clicked() {
                    if let Some(request) = session.request.clone() {
                        start.write(StartPuzzle(request));
                    }
                }
                if ui.button("Menu").clicked() {
                    next_state.set(GameState::MainMenu);
                }
            });
        });
}
