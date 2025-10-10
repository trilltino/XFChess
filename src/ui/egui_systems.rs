use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use crate::core::GameState;

pub fn playgame_ui(
    mut contexts: EguiContexts,
    mut next_state: ResMut<NextState<GameState>>,
) {
    let ctx = contexts.ctx_mut();

    egui::Window::new("XFChess")
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);
                
                ui.heading(egui::RichText::new("XFChess").size(48.0));
                
                ui.add_space(40.0);

                if ui.button(egui::RichText::new("Start Game").size(28.0)).clicked() {
                    next_state.set(GameState::Multiplayer);
                }

                ui.add_space(15.0);

                if ui.button(egui::RichText::new("Exit").size(28.0)).clicked() {
                    std::process::exit(0);
                }
            });
        });
}

pub struct UIPlugin;
impl Plugin for UIPlugin {
    fn build(&self, _app: &mut App) {
        // Empty - system is registered in launch_menu plugin
    }
}
