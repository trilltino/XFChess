//! Audio volume control system
//!
//! Applies master volume setting to all audio sinks.

use crate::core::GameSettings;
use bevy::audio::Volume;
use bevy::prelude::*;

/// System that applies master volume to all audio sinks
///
/// Watches for changes to `GameSettings.master_volume` and updates all AudioSink components.
pub fn apply_master_volume_system(
    settings: Res<GameSettings>,
    mut audio_sinks: Query<&mut AudioSink>,
    mut last_volume: Local<Option<f32>>,
) {
    // Check if volume changed
    let current_volume = settings.master_volume;
    if let Some(prev_volume) = *last_volume {
        if (prev_volume - current_volume).abs() < 0.001 {
            return; // No significant change
        }
    }
    *last_volume = Some(current_volume);

    // Apply volume to all audio sinks
    for mut sink in audio_sinks.iter_mut() {
        sink.set_volume(Volume::Linear(current_volume));
    }

    if !audio_sinks.is_empty() {
        info!(
            "[AUDIO] Applied master volume: {:.0}%",
            current_volume * 100.0
        );
    }
}
