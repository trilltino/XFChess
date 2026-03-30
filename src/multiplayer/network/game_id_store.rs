//! Global game-id store.
//!
//! A single `AtomicU64` that any thread can read without going through the ECS.
//! Set once when a game is created or joined; read by the move-broadcast path.

use std::sync::atomic::{AtomicU64, Ordering};

static ACTIVE_GAME_ID: AtomicU64 = AtomicU64::new(0);

/// Store the active game id.
pub fn set(id: u64) {
    ACTIVE_GAME_ID.store(id, Ordering::SeqCst);
}

/// Retrieve the active game id. Returns `0` if no game has started yet.
pub fn get() -> u64 {
    ACTIVE_GAME_ID.load(Ordering::SeqCst)
}
