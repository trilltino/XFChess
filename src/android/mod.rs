//! Android entry point and platform glue.

pub mod platform;

use crate::{build_app, GameConfig};
use bevy::prelude::*;

#[bevy_main]
pub fn main() {
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info),
    );

    // Must happen before any HTTP client is constructed — reqwest's `rustls`
    // feature routes Android TLS verification through a Kotlin class that
    // needs an Android Context first. See the Cargo.toml comment on the
    // rustls-platform-verifier dependency for why this is not optional.
    if let Err(e) = platform::init_tls_verifier() {
        log::error!("[android] rustls-platform-verifier init failed: {e}");
    }

    let mut app = build_app(GameConfig::default());
    app.run();
}

/// Pauses audio while Android suspends the app. The render loop is already
/// handled by Bevy's mobile settings; `Suspended` avoids reacting to transient
/// `WillSuspend` events.
pub fn handle_app_lifecycle(
    mut lifecycle_events: bevy::ecs::message::MessageReader<bevy::window::AppLifecycle>,
    sinks: Query<&bevy::audio::AudioSink>,
) {
    use bevy::audio::AudioSinkPlayback;

    for event in lifecycle_events.read() {
        match event {
            bevy::window::AppLifecycle::Suspended => {
                for sink in &sinks {
                    sink.pause();
                }
            }
            bevy::window::AppLifecycle::Running => {
                for sink in &sinks {
                    sink.play();
                }
            }
            _ => {}
        }
    }
}
