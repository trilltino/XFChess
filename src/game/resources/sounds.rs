//! Game sounds resource for chess move and capture audio feedback
//!
//! Loads and stores handles to game sound effects that are played during gameplay.

use bevy::audio::AudioSource;
use bevy::prelude::*;

/// Resource storing handles to all game sound effects
///
/// Sounds are loaded when entering InGame state and can be played
/// by systems that need audio feedback.
#[derive(Resource)]
pub struct GameSounds {
    /// Sound played when a piece moves
    pub move_piece: Handle<AudioSource>,
    /// Sound played when a piece is captured
    pub capture_piece: Handle<AudioSource>,
    /// Sound played when a king is in check
    pub check: Handle<AudioSource>,
    /// Sound played when an illegal move is attempted
    pub illegal: Handle<AudioSource>,
}

impl FromWorld for GameSounds {
    fn from_world(world: &mut World) -> Self {
        let asset_server = world.resource::<AssetServer>();
        Self {
            move_piece: asset_server.load("game_sounds/move_piece.mp3"),
            capture_piece: asset_server.load("game_sounds/capture_piece.mp3"),
            // These fall back to silent if files are missing
            check: asset_server.load("game_sounds/check.mp3"),
            illegal: asset_server.load("game_sounds/illegal.mp3"),
        }
    }
}

