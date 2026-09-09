
use crate::core::GameSettings;
use crate::rendering::utils::SquareMaterials;
use bevy::prelude::*;

pub fn update_board_theme_system(
    settings: Res<GameSettings>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    square_materials: Res<SquareMaterials>,
    mut last_theme: Local<Option<crate::core::BoardTheme>>,
) {
    // Check if theme changed
    let current_theme = settings.board_theme;
    if let Some(prev_theme) = *last_theme {
        if prev_theme == current_theme {
            return; // No change
        }
    }
    *last_theme = Some(current_theme);

    let (light_color, dark_color) = settings.board_theme.colors();

    // Update material assets
    if let Some(light_mat) = materials.get_mut(&square_materials.black_color) {
        light_mat.base_color = light_color;
    }
    if let Some(dark_mat) = materials.get_mut(&square_materials.white_color) {
        dark_mat.base_color = dark_color;
    }

    info!(
        "[BOARD_THEME] Updated board theme to {:?}",
        settings.board_theme.name()
    );
}
