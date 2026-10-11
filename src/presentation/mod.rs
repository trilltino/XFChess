//! Audio output: the master volume and mute settings.
//!
//! Bevy scales every sound by [`GlobalVolume`] when it starts playing, so new
//! sounds pick the setting up on their own; when the setting changes, sounds
//! already playing are rescaled too. Sounds themselves are triggered by the
//! features that own them.

use bevy::audio::Volume;
use bevy::prelude::*;

use crate::core::GameSettings;

pub struct PresentationPlugin;

impl Plugin for PresentationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            apply_master_volume
                .in_set(crate::core::schedule::ApplySettings)
                .run_if(resource_exists_and_changed::<GameSettings>),
        );
    }
}

fn apply_master_volume(
    settings: Res<GameSettings>,
    mut global: ResMut<GlobalVolume>,
    mut playing: Query<(&mut AudioSink, &PlaybackSettings)>,
) {
    let master = if settings.muted {
        0.0
    } else {
        settings.master_volume
    };
    let volume = Volume::Linear(master);
    // Settings change for many reasons; only volume/mute matter here.
    if global.volume == volume {
        return;
    }
    global.volume = volume;
    for (mut sink, playback) in &mut playing {
        sink.set_volume(playback.volume * volume);
    }
    debug!("[AUDIO] Master volume {:.0}%", master * 100.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App {
        let mut app = App::new();
        app.init_resource::<GlobalVolume>()
            .init_resource::<GameSettings>()
            .add_plugins(PresentationPlugin);
        app.update();
        app
    }

    fn global(app: &App) -> Volume {
        app.world().resource::<GlobalVolume>().volume
    }

    #[test]
    fn global_volume_follows_the_setting_from_the_start() {
        let app = app();
        let expected = app.world().resource::<GameSettings>().master_volume;
        assert_eq!(global(&app), Volume::Linear(expected));
    }

    #[test]
    fn changing_volume_or_mute_updates_every_new_sound() {
        let mut app = app();
        app.world_mut().resource_mut::<GameSettings>().master_volume = 0.25;
        app.update();
        assert_eq!(global(&app), Volume::Linear(0.25));

        app.world_mut().resource_mut::<GameSettings>().muted = true;
        app.update();
        assert_eq!(global(&app), Volume::Linear(0.0));

        app.world_mut().resource_mut::<GameSettings>().muted = false;
        app.update();
        assert_eq!(global(&app), Volume::Linear(0.25));
    }
}