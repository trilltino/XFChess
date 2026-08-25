//! Android entry point and platform glue.
//!
//! `#[bevy_main]` (not a hand-written `android_main`) matches Bevy's own
//! `examples/mobile/src/lib.rs` exactly: it expands to an `android_main` that
//! stashes the `AndroidApp` in `bevy::android::ANDROID_APP` and calls this
//! function. It requires only that the function be named `main` — it does not
//! require a binary crate, and this `pub fn main` does not collide with
//! `src/main.rs`'s desktop entry point, which is never compiled for Android
//! (see the `[[bin]]` targets in the root `Cargo.toml`, none of which are
//! built by `cargo ndk`).

// Named `platform`, not `jni` — a sibling module named `jni` would shadow the
// external `jni` crate for any bare `use jni::...` written directly in this
// file's own scope (Rust 2018+ resolves same-scope items before the extern
// prelude). `mwa.rs`, added later, needs the real crate extensively.
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
        // Nothing else has a logger set up yet to escalate this further than
        // a log line — every HTTPS call after this point will fail at the
        // handshake if it did not succeed, which is loud on its own.
        log::error!("[android] rustls-platform-verifier init failed: {e}");
    }

    // No argv on Android — GameConfig::Default mirrors desktop's fallback
    // when no CLI args are parsed (see GameConfig's own Default impl).
    let mut app = build_app(GameConfig::default());
    app.run();
}

/// Pauses/resumes every live `AudioSink` on Android app backgrounding —
/// mirrors Bevy's own `examples/mobile/src/lib.rs` `handle_lifetime` system.
///
/// The render-loop half of "don't burn battery backgrounded" is already
/// covered by `WinitSettings::mobile()` (see `lib.rs`) and, more
/// fundamentally, by winit's own event loop simply not calling `Update` at
/// all while suspended — so this system only has audio left to actually
/// handle, since a sound can keep playing under `rodio` even once nothing
/// is polling ECS systems to stop it. There is deliberately no "pause the
/// AI search task" here: `bevy::tasks::Task` has no public pause/cancel API,
/// and once `Update` stops being called (which suspension already implies)
/// nothing polls the task to apply its result anyway — the search just
/// finishes quietly in the background at worst, not a correctness issue.
///
/// `WillSuspend`/`WillResume` give the app exactly one frame to react (see
/// `AppLifecycle`'s own doc comment) — pausing on `Suspended` rather than
/// `WillSuspend` is deliberate: the transition is still one frame away, but
/// reacting on the state that's about to become durable rather than the
/// one-frame warning avoids a spurious pause/resume flicker if the OS ever
/// sends `WillSuspend` without immediately following through.
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
