//! Presentation component entry point
pub mod audio;

use audio::AudioPresentationPlugin;
use bevy::prelude::*;

pub struct PresentationPlugin;

impl Plugin for PresentationPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(AudioPresentationPlugin);
    }
}
