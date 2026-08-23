//! Live spectator feed for on-chain games, over Braid-HTTP 209.
//!
//! ```text
//! GET /spectate/{game_id}/moves    209 subscribe (or 200 snapshot)
//! ```
//!
//! # What this replaces
//!
//! Spectating used to be a 2-second poll of `GET /games/moves/{id}` that
//! re-fetched the whole move list and diffed it client-side against a local
//! `applied_move_count`. On a game whose moves land sub-second on the ER, that
//! is up to 2s of added latency per move, and the cost grows with game length
//! times spectators.
//!
//! Here a spectator subscribes once and receives **the moves so far, then each
//! new move as it is recorded** — which is what watching a game in progress
//! actually is, and it removes the catch-up path entirely.
//!
//! # Why not `game_log.rs`
//!
//! That module serves the same shape for *casual* games, but its `put_event` is
//! built for client-authored writes: it validates a causal chain against
//! caller-supplied `content_version`/`content_parent` and checks the poster is a
//! participant. On-chain moves arrive server-side through `record_move`, which
//! has already proved participation on-chain. Pushing them through `put_event`
//! would mean synthesizing version hashes to satisfy an invariant that exists to
//! constrain untrusted clients. These are separate resources on purpose.
//!
//! # The broadcast delay
//!
//! A tournament game may carry a non-zero `broadcast_delay_secs`, and the
//! polled feed filters moves older than that horizon ([`filter_visible_moves`]).
//! A live subscription would walk straight around that filter, so **a delayed
//! game is never streamed here**: the route answers `404`, and the client falls
//! back to the delay-gated poll it already implements. `record_move` applies the
//! same check before publishing, so the resource for a delayed game never even
//! exists.
//!
//! [`filter_visible_moves`]: crate::db::repository::filter_visible_moves

use axum::{
    body::Body,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use braid_chess::{ChessMessage, MovePayload};
use bytes::Bytes;
use futures::StreamExt;
use std::time::Duration;
use tokio::time::interval;
use tokio_stream::wrappers::BroadcastStream;
use tracing::{debug, warn};
use xfchess_braid_server::resource::protocol::{format_chunk, format_heartbeat, BraidUpdate};

use crate::db::repository::GameRepository;
use crate::signing::routes::main::spectator_moves_path;
use crate::signing::AppState;

/// How often an idle subscription proves it is alive.
const HEARTBEAT_SECS: u64 = 20;

/// Whether a game's moves may be streamed live.
///
/// The anti-ghosting control, kept as one pure function because both sides of
/// the guard have to agree: this route refuses to open a subscription, and
/// `record_move` refuses to publish into the resource at all. Pure so the
/// decision is unit-testable without a database or an HTTP stack — the same
/// reason `spectator::feed_is_delayed` is pure on the client.
pub fn is_streamable(broadcast_delay_secs: i64) -> bool {
    broadcast_delay_secs <= 0
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/spectate/{game_id}/moves", get(get_spectator_moves))
}

async fn get_spectator_moves(
    State(state): State<AppState>,
    Path(game_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let repo = GameRepository::new(state.store.pool());

    // Delayed games are not streamable — see the module doc. The client reads
    // this 404 as "use the delayed poll", which is also its fail-safe default.
    if !is_streamable(repo.get_broadcast_delay(&game_id).await) {
        return (
            StatusCode::NOT_FOUND,
            "game has a broadcast delay; use GET /games/moves/{id}",
        )
            .into_response();
    }

    let path = spectator_moves_path(&game_id);

    // Hydrate from durable storage if this game's log isn't in memory — the
    // process may have restarted mid-game, and a spectator joining then must
    // still see the moves already played, not just the next one.
    if state.braid_hub.ensure_log(&path) {
        match repo.get_moves(&game_id).await {
            Ok(moves) => {
                debug!(
                    "[spectate] hydrated {} with {} persisted moves",
                    path,
                    moves.len()
                );
                for m in moves {
                    state.braid_hub.append(&path, move_record_to_json(&m));
                }
            }
            Err(e) => warn!("[spectate] could not hydrate {}: {}", path, e),
        }
    }

    if !braid_chess::braid_http::server::wants_subscribe(|name| {
        headers.get(name).and_then(|v| v.to_str().ok())
    }) {
        let snapshot = state
            .braid_hub
            .current_json(&path)
            .await
            .unwrap_or_else(|| serde_json::Value::Array(Vec::new()));
        return (StatusCode::OK, Json(snapshot)).into_response();
    }

    let Some((snapshot, rx)) = state.braid_hub.subscribe(&path).await else {
        return StatusCode::NOT_FOUND.into_response();
    };

    // One chunk per historical move, each shaped exactly like a live update
    // body (a bare `ChessMessage`), so `braid_chess::ChessSubscriber` decodes
    // history and tail through one code path. A single bulk-array snapshot
    // chunk would not decode — see `game_log.rs`'s regression test.
    let history_chunks: Vec<Bytes> = snapshot
        .body
        .as_array()
        .map(|entries| {
            entries
                .iter()
                .enumerate()
                .map(|(i, entry)| format_chunk(&BraidUpdate::snapshot(i as u64, entry.clone())))
                .collect()
        })
        .unwrap_or_default();

    let hb_chunk = format_heartbeat();
    let rx_stream = BroadcastStream::new(rx);
    let path_owned = path.clone();

    let body_stream = async_stream::stream! {
        debug!(
            "[spectate] subscriber connected to {} ({} historical moves)",
            path_owned, history_chunks.len()
        );
        for chunk in history_chunks {
            yield Ok::<Bytes, String>(chunk);
        }

        let mut ticker = interval(Duration::from_secs(HEARTBEAT_SECS));
        ticker.tick().await;

        tokio::pin!(rx_stream);
        loop {
            tokio::select! {
                maybe_update = rx_stream.next() => {
                    match maybe_update {
                        // A live append arrives as an `add /-` patch carrying
                        // the one new move; re-emit it as a bare snapshot body
                        // so it decodes identically to the history above.
                        Some(Ok(update)) => {
                            if let Some(entry) = appended_entry(&update) {
                                yield Ok(format_chunk(&BraidUpdate::snapshot(update.version, entry)));
                            }
                        }
                        Some(Err(e)) => warn!("[spectate] broadcast lag on {}: {}", path_owned, e),
                        None => break,
                    }
                }
                _ = ticker.tick() => yield Ok(hb_chunk.clone()),
            }
        }
        debug!("[spectate] subscriber disconnected from {}", path_owned);
    };

    Response::builder()
        .status(209)
        .header("Content-Type", "application/http-history")
        .header("Cache-Control", "no-store")
        .header("Heartbeats", HEARTBEAT_SECS.to_string())
        .body(Body::from_stream(body_stream))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

/// Pull the appended value out of an `AppendLog` update's `add /-` patch.
fn appended_entry(update: &BraidUpdate) -> Option<serde_json::Value> {
    if update.is_snapshot {
        return Some(update.body.clone());
    }
    update
        .body
        .as_array()
        .and_then(|ops| ops.first())
        .and_then(|op| op.get("value"))
        .cloned()
}

/// A persisted move as the same `ChessMessage::Move` a live append carries.
fn move_record_to_json(m: &crate::db::repository::MoveRecord) -> serde_json::Value {
    let payload = MovePayload::from_uci(
        m.move_uci.clone(),
        m.fen_after.clone().unwrap_or_default(),
        m.move_number.max(0) as u32,
        m.player.clone(),
    );
    serde_json::to_value(ChessMessage::Move(payload)).unwrap_or(serde_json::Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The anti-ghosting control. A tournament game with a broadcast delay
    /// must never be streamable: the polled feed hides moves newer than the
    /// delay horizon, and a live subscription would hand them over instantly.
    ///
    /// Both the subscribe route and `record_move`'s publish call this, so a
    /// regression here opens the hole in two places at once.
    #[test]
    fn a_delayed_game_is_never_streamable() {
        assert!(is_streamable(0), "a live game streams");
        assert!(
            is_streamable(-1),
            "a nonsensical negative delay is not a gate"
        );

        assert!(!is_streamable(1), "even a 1s delay must block the stream");
        assert!(!is_streamable(30));
        assert!(!is_streamable(900));
    }

    #[test]
    fn a_live_append_unwraps_to_the_move_itself() {
        let entry = json!({ "type": "Move", "uci": "e2e4" });
        let update =
            BraidUpdate::patch(2, 1, json!([{ "op": "add", "path": "/-", "value": entry }]));
        assert_eq!(appended_entry(&update), Some(entry));
    }

    /// History and tail must decode through the same client path, so a
    /// persisted move has to serialize to the same shape a live one does.
    #[test]
    fn a_persisted_move_matches_the_live_message_shape() {
        let record = crate::db::repository::MoveRecord {
            id: Some(1),
            game_id: "7".into(),
            move_number: 3,
            move_uci: "e7e8q".into(),
            move_san: Some("e8=Q".into()),
            fen_before: None,
            fen_after: Some("8/8/8/8/8/8/8/4Q3 b - - 0 3".into()),
            player: "alice".into(),
            timestamp: 0,
        };

        let encoded = move_record_to_json(&record);
        let decoded: ChessMessage =
            serde_json::from_value(encoded).expect("must decode as a ChessMessage");

        match decoded {
            ChessMessage::Move(p) => {
                assert_eq!(p.uci, "e7e8q");
                assert_eq!(p.from, "e7");
                assert_eq!(p.to, "e8");
                assert_eq!(p.promotion, Some('q'));
                assert_eq!(p.move_number, 3);
            }
            other => panic!("unexpected message: {other:?}"),
        }
    }
}
