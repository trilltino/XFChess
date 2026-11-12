//! Android entry point (`#[bevy_main]`) and platform glue: logging, the rustls
//! platform verifier, and JNI helpers in [`platform`]. The Gradle shell lives in
//! `apps/android/`.

use crate::{build_app, GameConfig};
use bevy::prelude::*;

#[bevy_main]
pub fn main() {
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info),
    );

    // Initialize AndroidContext before creating clients that use the platform verifier.
    if let Err(e) = crate::platform::android::init_tls_verifier() {
        log::error!("[android] rustls-platform-verifier init failed: {e}");
        return;
    }

    let mut app = match build_app(GameConfig::default()) {
        Ok(app) => app,
        Err(error) => {
            log::error!("[android] application startup failed: {error}");
            return;
        }
    };
    app.run();
}
