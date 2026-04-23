//! Bridge: converts XFChess backend mutations into Braid resource updates.
//!
//! The backend calls these free functions after mutating `TournamentStore` /
//! `SwissService`. Each function builds the appropriate JSON Patch and calls
//! into the [`ResourceHub`].
//!
//! No async required — `ResourceHub::patch` and `replace` are synchronous.

use crate::ResourceHub;
use serde_json::{json, Value};

/// Push a full tournament meta update (after create / status change).
pub fn push_tournament_meta(hub: &ResourceHub, tournament_id: u64, meta: Value) {
    hub.ensure_tournament(tournament_id);
    hub.replace(&format!("tournament/{}/meta", tournament_id), meta);
}

/// Push a schedule-status update.
pub fn push_schedule_status(hub: &ResourceHub, tournament_id: u64, status: Value) {
    hub.ensure_tournament(tournament_id);
    hub.replace(&format!("tournament/{}/schedule-status", tournament_id), status);
}

/// Push the full roster after a player registers.
pub fn push_roster(hub: &ResourceHub, tournament_id: u64, players: &[String]) {
    hub.ensure_tournament(tournament_id);
    let roster: Vec<Value> = players.iter().map(|p| json!(p)).collect();
    hub.replace(&format!("tournament/{}/roster", tournament_id), Value::Array(roster));
}

/// Push full standings after a result is recorded.
pub fn push_standings(hub: &ResourceHub, tournament_id: u64, standings: Value) {
    hub.ensure_tournament(tournament_id);
    hub.replace(&format!("tournament/{}/standings", tournament_id), standings);
}

/// Push pairings for a round after it starts.
pub fn push_pairings(hub: &ResourceHub, tournament_id: u64, round: u8, pairings: Value) {
    hub.ensure_tournament(tournament_id);
    hub.ensure_pairings(tournament_id, round);
    hub.replace(&format!("tournament/{}/pairings/{}", tournament_id, round), pairings);
}
