use crate::egui::RichText;
use bevy::prelude::*;
use bevy_egui::{
    egui::{self, Color32, Vec2, Widget},
    EguiContextPass, EguiContexts, EguiPlugin,
};

use crate::state_manager::GameState;

pub fn playgame_ui(mut contexts: EguiContexts, mut next_state: Res<State<GameState>>) {
    let ctx = contexts.ctx_mut();
    egui::Window::new("Launch Menu")
        .default_pos((100.0, 100.0))
        .default_size((520.0, 300.0))
        .resizable(true)
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                let button_size = Vec2::new(500.0, 60.0);

                let multiplayer_button =
                    egui::Button::new(RichText::new("Multiplayer").color(Color32::WHITE).strong())
                        .min_size(button_size)
                        .fill(Color32::from_rgb(30, 144, 255));

                let settings_button =
                    egui::Button::new(RichText::new("Settings").color(Color32::WHITE).strong())
                        .min_size(button_size)
                        .fill(Color32::from_rgb(30, 144, 255));

                let exit_button =
                    egui::Button::new(RichText::new("Exit Game").color(Color32::WHITE).strong())
                        .min_size(button_size)
                        .fill(Color32::from_rgb(30, 144, 255));

                if ui.add(multiplayer_button).clicked() {
                    next_state.set(GameState::Multiplayer);
                    println!("Multiplayer pressed")
                }

                if ui.add(settings_button).clicked() {
                    println!("Settings pressed");
                }

                if ui.add(exit_button).clicked() {
                    println!("Game Exited")
                }
            });
        });
}

pub struct UIPlugin;
impl Plugin for UIPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(EguiContextPass, playgame_ui);
    }
}
