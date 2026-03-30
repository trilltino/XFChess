//! TempleOS UI elements
//!
//! Displays the Terry A. Davis quote and attribution at the bottom of the screen
//! when in TempleOS view mode.

use crate::core::{DespawnOnExit, GameState};
use crate::game::view_mode::ViewMode;
use bevy::prelude::*;

/// Marker component for TempleOS UI text
#[derive(Component)]
pub struct TempleOSQuote;

/// System to create TempleOS quote UI at the bottom of the screen
pub fn create_templeos_quote_ui(mut commands: Commands, view_mode: Res<ViewMode>) {
    // Only create UI in TempleOS mode
    if *view_mode != ViewMode::TempleOS {
        return;
    }

    // Spawn the quote text at the bottom of the screen
    // Use Text with multiple lines for proper wrapping
    commands.spawn((
        Text::new("there's nothing better to do with your time than kill time with Mr. God and enjoy divine intellect all day long."),
        TextFont {
            font_size: 14.0,
            ..default()
        },
        TextColor(Color::srgb(0.0, 0.0, 0.0)), // Black text
        TextLayout::new_with_justify(bevy::text::Justify::Center),
        Node {
            position_type: PositionType::Absolute,
            bottom: px(50),
            left: px(20),
            right: px(20),
            max_width: Val::Percent(90.0),
            ..default()
        },
        TempleOSQuote,
        DespawnOnExit(GameState::InGame),
        Name::new("TempleOS Quote"),
    ));

    // Spawn Terry A. Davis attribution
    commands.spawn((
        Text::new("Terry.A.Davis"),
        TextFont {
            font_size: 12.0,
            ..default()
        },
        TextColor(Color::srgb(0.0, 0.0, 0.0)), // Black text
        TextLayout::new_with_justify(bevy::text::Justify::Center),
        Node {
            position_type: PositionType::Absolute,
            bottom: px(40),
            left: px(20),
            right: px(20),
            ..default()
        },
        TempleOSQuote,
        DespawnOnExit(GameState::InGame),
        Name::new("TempleOS Attribution Name"),
    ));

    // Spawn years
    commands.spawn((
        Text::new("1969 - 2018"),
        TextFont {
            font_size: 12.0,
            ..default()
        },
        TextColor(Color::srgb(0.0, 0.0, 0.0)), // Black text
        TextLayout::new_with_justify(bevy::text::Justify::Center),
        Node {
            position_type: PositionType::Absolute,
            bottom: px(25),
            left: px(20),
            right: px(20),
            ..default()
        },
        TempleOSQuote,
        DespawnOnExit(GameState::InGame),
        Name::new("TempleOS Attribution Years"),
    ));

    info!("[TEMPLEOS_UI] Created quote and attribution UI");
}
