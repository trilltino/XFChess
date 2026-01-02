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
    /// Sound played when entering TempleOS view
    pub temple_os: Handle<AudioSource>,
}

impl GameSounds {
    /// Load all game sounds from the asset server
    pub fn new(asset_server: &AssetServer) -> Self {
        Self {
            move_piece: asset_server.load("game_sounds/move_piece.mp3"),
            capture_piece: asset_server.load("game_sounds/capture_piece.mp3"),
            temple_os: asset_server.load("game_sounds/board_themes/temple_os.mp3"),
        }
    }
}
