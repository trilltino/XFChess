//! Tournament multiplayer module — **DEAD CODE. NOT WIRED UP.**
//!
//! Intended to provide real-time tournament updates over braid-iroh gossip,
//! but [`TournamentMultiplayerPlugin`] is **never registered with the Bevy
//! app** — grep it: the only hits are its own definition. Nothing in here has
//! ever run.
//!
//! The live tournament client is
//! [`crate::multiplayer::solana::tournament::TournamentClientPlugin`], which
//! polls the backend over HTTP (`/my-status`, see
//! `docs/plans/tournament-end-to-end-fix-plan.md`). That is authoritative.
//!
//! **Name collision warning:** this module also exports a type called
//! `TournamentClientPlugin` (re-exported from `client.rs`). It is *not* the
//! live one. Importing it by name from here registers a plugin that does
//! nothing, in place of the one that does everything. Prefer the fully
//! qualified `solana::tournament::TournamentClientPlugin`.
//!
//! Either wire this up as the live-update transport with polling as fallback,
//! or delete the module — leaving a second, silently-dead implementation of
//! the same concept is the worst of both. Tracked as item 13 in the plan above.

pub mod client;
pub mod events;

pub use client::TournamentClientPlugin;
pub use events::TournamentEventsPlugin;

use bevy::prelude::*;

/// Combined plugin for tournament multiplayer
pub struct TournamentMultiplayerPlugin;

impl Plugin for TournamentMultiplayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((TournamentClientPlugin, TournamentEventsPlugin));
    }
}
