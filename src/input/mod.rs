//! Player input on the board: picking observers for selecting and moving pieces
//! ([`board`]), hover feedback and the cursor icon ([`pointer`]), plus the
//! in-game keys (fullscreen, Escape to leave).
//!
//! Bevy's picking backends come with `DefaultPlugins`; the observers here are
//! attached to piece and square entities when they spawn
//! (`rendering::pieces`, `rendering::board`). See [README.md](README.md).

pub mod board;
pub mod pointer;

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::core::states::InGameplay;
use pointer::CursorStyle;

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CursorStyle>()
            .add_systems(
                Update,
                apply_cursor_icon.run_if(resource_changed::<CursorStyle>),
            )
            .add_systems(OnExit(InGameplay), reset_cursor);
    }
}

/// Shows the hover cursor chosen by the pointer observers on the game window.
fn apply_cursor_icon(
    style: Res<CursorStyle>,
    window: Option<Single<Entity, With<PrimaryWindow>>>,
    mut commands: Commands,
) {
    if let Some(window) = window {
        commands.entity(*window).insert(style.current.clone());
    }
}

/// Hovered pieces and squares are despawned with the board, so forget them.
fn reset_cursor(mut style: ResMut<CursorStyle>) {
    style.active_hovers.clear();
    style.update();
}
