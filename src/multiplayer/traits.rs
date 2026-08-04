//! Compatibility re-exports for Bevy 0.18 message primitives.
//!
//! Bevy 0.18 introduced the `Message` trait (distinct from `Event`) along with
//! `MessageReader`, `MessageWriter`, and `App::add_message` as the buffered-event
//! replacements for the previous `Event`/`EventReader`/`EventWriter`/`add_event`
//! API. This module simply forwards those names so the rest of the crate can
//! continue to import them from `crate::multiplayer::traits`.

pub use bevy::prelude::{Message, MessageReader, MessageWriter};
