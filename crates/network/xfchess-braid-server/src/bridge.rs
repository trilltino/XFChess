use crate::ResourceHub;
use serde_json::{json, Value};

pub fn push_tournament_meta(hub: &ResourceHub, tournament_id: u64, meta: Value) {
    hub.ensure_tournament(tournament_id);
    hub.replace(&format!("tournament/{}/meta", tournament_id), meta);
}

pub fn push_schedule_status(hub: &ResourceHub, tournament_id: u64, status: Value) {
    hub.ensure_tournament(tournament_id);
    hub.replace(
        &format!("tournament/{}/schedule-status", tournament_id),
        status,
    );
}

pub fn push_bracket_fired(
    hub: &ResourceHub,
    tournament_id: u64,
    player_count: u16,
    started_at: i64,
) {
    push_schedule_status(
        hub,
        tournament_id,
        json!({
            "status": "started",
            "player_count": player_count,
            "started_at": started_at,
        }),
    );
}

pub fn push_result(hub: &ResourceHub, tournament_id: u64, round: u8, board: u16, result: Value) {
    hub.ensure_tournament(tournament_id);
    hub.append(
        &format!("tournament/{}/results", tournament_id),
        json!({
            "round": round,
            "board": board,
            "result": result,
        }),
    );
}

pub fn push_roster(hub: &ResourceHub, tournament_id: u64, players: &[String]) {
    hub.ensure_tournament(tournament_id);
    let roster: Vec<Value> = players.iter().map(|p| json!(p)).collect();
    hub.replace(
        &format!("tournament/{}/roster", tournament_id),
        Value::Array(roster),
    );
}

pub fn push_standings(hub: &ResourceHub, tournament_id: u64, standings: Value) {
    hub.ensure_tournament(tournament_id);
    hub.replace(
        &format!("tournament/{}/standings", tournament_id),
        standings,
    );
}

pub fn push_pairings(hub: &ResourceHub, tournament_id: u64, round: u8, pairings: Value) {
    hub.ensure_tournament(tournament_id);
    hub.ensure_pairings(tournament_id, round);
    hub.replace(
        &format!("tournament/{}/pairings/{}", tournament_id, round),
        pairings,
    );
}
