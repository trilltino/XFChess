//! Presentation layer - Audio systems
//!
//! Handles all audio playback and volume management for the application.

use crate::core::GameSettings;
use bevy::audio::Volume;
use bevy::prelude::*;

/// Plugin for the presentation layer audio systems
pub struct AudioPresentationPlugin;

impl Plugin for AudioPresentationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, apply_master_volume_system);
    }
}

/// System that applies master volume to all audio sinks
///
/// Watches for changes to `GameSettings.master_volume` and updates all AudioSink components.
pub fn apply_master_volume_system(
    settings: Res<GameSettings>,
    mut audio_sinks: Query<&mut AudioSink>,
    mut last_settings: Local<Option<(f32, bool)>>,
) {
    // Check if volume or mute changed
    let current_volume = settings.master_volume;
    let current_muted = settings.muted;

    if let Some((prev_volume, prev_muted)) = *last_settings {
        if (prev_volume - current_volume).abs() < 0.001 && prev_muted == current_muted {
            return; // No significant change
        }
    }
    *last_settings = Some((current_volume, current_muted));

    // Calculate effective volume
    let effective_volume = if current_muted { 0.0 } else { current_volume };

    // Apply volume to all audio sinks
    for mut sink in audio_sinks.iter_mut() {
        sink.set_volume(Volume::Linear(effective_volume));
    }

    if !audio_sinks.is_empty() {
        info!(
            "[AUDIO] Applied volume: {:.0}% (Muted: {})",
            effective_volume * 100.0,
            current_muted
        );
    }
}
