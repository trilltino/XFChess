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

/// Spawn a one-shot sound effect that despawns itself once playback finishes.
///
/// Always use this instead of `commands.spawn(AudioPlayer::new(..))` for SFX.
/// Bevy's default `PlaybackSettings` is `PlaybackMode::Once`, which plays the
/// clip and then does *nothing* — the entity and its live `AudioSink` survive
/// for the rest of the process. Each of those sinks is a `rodio` player built on
/// `queue::queue(true)`, i.e. a source that emits silence forever instead of
/// ending, so rodio's mixer never drops it from `current_sources`. The audio
/// thread then re-mixes one extra dead source *per sample* for every sound ever
/// played: after a long session that is thousands of virtual calls at 44.1 kHz,
/// which saturates a core and starves the render thread.
///
/// `PlaybackMode::Despawn` lets `cleanup_finished_audio` drop the entity — and
/// with it the sink — as soon as the clip drains.
pub fn play_sfx(commands: &mut Commands, sound: Handle<AudioSource>) {
    commands.spawn((AudioPlayer::new(sound), PlaybackSettings::DESPAWN));
}
