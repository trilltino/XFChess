use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use crate::core::GameState;
use crate::rendering::pieces::PieceColor;
use crate::game::ai::{ChessAIResource, GameMode, AIDifficulty};

pub fn playgame_ui(
    mut contexts: EguiContexts,
    mut next_state: ResMut<NextState<GameState>>,
    mut ai_config: ResMut<ChessAIResource>,
) -> Result<(), bevy::ecs::query::QuerySingleError> {
    let ctx = contexts.ctx_mut()?;

    egui::Window::new("XFChess")
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);

                ui.heading(egui::RichText::new("XFChess").size(48.0));

                ui.add_space(40.0);

                // Game mode selection
                ui.heading(egui::RichText::new("Select Game Mode").size(24.0));
                ui.add_space(20.0);

                if ui.button(egui::RichText::new("Human vs Human").size(20.0)).clicked() {
                    ai_config.mode = GameMode::VsHuman;
                    next_state.set(GameState::Multiplayer);
                }

                ui.add_space(10.0);

                if ui.button(egui::RichText::new("Human (White) vs AI (Black)").size(20.0)).clicked() {
                    ai_config.mode = GameMode::VsAI { ai_color: PieceColor::Black };
                    next_state.set(GameState::Multiplayer);
                }

                ui.add_space(10.0);

                if ui.button(egui::RichText::new("AI (White) vs Human (Black)").size(20.0)).clicked() {
                    ai_config.mode = GameMode::VsAI { ai_color: PieceColor::White };
                    next_state.set(GameState::Multiplayer);
                }

                ui.add_space(30.0);

                // AI Difficulty selection
                ui.heading(egui::RichText::new("AI Difficulty").size(20.0));
                ui.add_space(10.0);

                ui.horizontal(|ui| {
                    ui.radio_value(&mut ai_config.difficulty, AIDifficulty::Easy, "Easy (0.5s)");
                    ui.radio_value(&mut ai_config.difficulty, AIDifficulty::Medium, "Medium (1.5s)");
                    ui.radio_value(&mut ai_config.difficulty, AIDifficulty::Hard, "Hard (3.0s)");
                });

                ui.add_space(40.0);

                if ui.button(egui::RichText::new("Exit").size(20.0)).clicked() {
                    std::process::exit(0);
                }
            });
        });

    Ok(())
}
