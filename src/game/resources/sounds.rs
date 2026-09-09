use bevy::audio::AudioSource;
use bevy::prelude::*;

#[derive(Resource)]
pub struct GameSounds {
    pub move_piece: Handle<AudioSource>,
    pub capture_piece: Handle<AudioSource>,
    pub check: Handle<AudioSource>,
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

pub fn play_sfx(commands: &mut Commands, sound: Handle<AudioSource>) {
    commands.spawn((AudioPlayer::new(sound), PlaybackSettings::DESPAWN));
}
