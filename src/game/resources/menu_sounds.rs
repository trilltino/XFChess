use bevy::audio::AudioSource;
use bevy::prelude::*;

#[derive(Resource)]
pub struct MenuSounds {
    pub menu_click: Handle<AudioSource>,
}

impl FromWorld for MenuSounds {
    fn from_world(world: &mut World) -> Self {
        let asset_server = world.resource::<AssetServer>();
        Self {
            menu_click: asset_server.load("menu_click.mp3"),
        }
    }
}
