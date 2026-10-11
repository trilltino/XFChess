use super::{belongs_to_stream, wants_subscribe, AppState, GameLogState, HEARTBEAT_SECS};
use axum::{
    body::Body,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use braid_chess::braid_http::{client::BraidHeaders, Version};
use bytes::Bytes;
use std::{sync::Arc, time::Duration};
use tokio::sync::broadcast;
use xfchess_braid_server::resource::protocol::{format_chunk, format_heartbeat, BraidUpdate};

#[derive(Debug)]
enum ResumeError {
    Gone,
    Database(sqlx::Error),
}

impl From<sqlx::Error> for ResumeError {
    fn from(error: sqlx::Error) -> Self {
        Self::Database(error)
    }
}

impl GameLogState {
    async fn resume_cursor(
        &self,
        game_id: &str,
        stream: &str,
        parents: &[Version],
    ) -> Result<i64, ResumeError> {
        let mut cursor = 0;
        for parent in parents {
            let seq = parent
                .as_integer()
                .or_else(|| parent.as_str().and_then(|value| value.parse::<i64>().ok()))
                .filter(|seq| *seq >= 0)
                .ok_or(ResumeError::Gone)?;
            if seq != 0 {
                let kind: Option<String> = sqlx::query_scalar(
                    "SELECT kind FROM game_event_log WHERE game_id = ? AND seq = ?",
                )
                .bind(game_id)
                .bind(seq)
                .fetch_optional(&self.pool)
                .await?;
                if !kind.is_some_and(|kind| belongs_to_stream(&kind, stream)) {
                    return Err(ResumeError::Gone);
                }
            }
            cursor = cursor.max(seq);
        }
        Ok(cursor)
    }

    async fn updates_after(
        &self,
        game_id: &str,
        stream: &str,
        after: i64,
    ) -> Result<Vec<BraidUpdate>, sqlx::Error> {
        let rows: Vec<(i64, String)> = sqlx::query_as(
            "SELECT seq, payload_json FROM game_event_log WHERE game_id = ? AND seq > ? AND ((? = 'chat' AND kind = 'chat') OR (? != 'chat' AND kind != 'chat')) ORDER BY seq ASC"
        ).bind(game_id).bind(after).bind(stream).bind(stream).fetch_all(&self.pool).await?;
        let mut parent = after as u64;
        rows.into_iter()
            .map(|(seq, payload)| {
                let body =
                    serde_json::from_str(&payload).map_err(|e| sqlx::Error::Decode(Box::new(e)))?;
                let mut update = BraidUpdate::snapshot(seq as u64, body);
                update.parents = vec![parent];
                parent = seq as u64;
                Ok(update)
            })
            .collect()
    }

    async fn subscribe_from(
        &self,
        game_id: &str,
        stream: &str,
        parents: &[Version],
    ) -> Result<(Vec<BraidUpdate>, broadcast::Receiver<BraidUpdate>, i64), ResumeError> {
        let rx = self.channel(game_id, stream).await.subscribe();
        let cursor = self.resume_cursor(game_id, stream, parents).await?;
        let history = self.updates_after(game_id, stream, cursor).await?;
        Ok((history, rx, cursor))
    }
}

pub(super) async fn get_stream(
    state: &AppState,
    game_id: &str,
    stream: &str,
    headers: HeaderMap,
) -> Response {
    stream_response(state.game_log.clone(), game_id, stream, headers).await
}

async fn stream_response(
    log: Arc<GameLogState>,
    game_id: &str,
    stream: &str,
    headers: HeaderMap,
) -> Response {
    if !wants_subscribe(&headers) {
        return (StatusCode::OK, Json(log.snapshot(game_id, stream).await)).into_response();
    }
    let parsed = match BraidHeaders::from_header_map(&headers) {
        Ok(parsed) if parsed.version.is_none() => parsed,
        _ => return StatusCode::BAD_REQUEST.into_response(),
    };
    let (history, mut rx, mut cursor) = match log
        .subscribe_from(
            game_id,
            stream,
            parsed.parents.as_deref().unwrap_or_default(),
        )
        .await
    {
        Ok(subscription) => subscription,
        Err(ResumeError::Gone) => return StatusCode::GONE.into_response(),
        Err(ResumeError::Database(error)) => {
            tracing::warn!("[game-log] could not open stream: {error}");
            return StatusCode::SERVICE_UNAVAILABLE.into_response();
        }
    };
    let game_id = game_id.to_owned();
    let stream = stream.to_owned();
    let body = async_stream::stream! {
        for update in history {
            cursor = update.version as i64;
            yield Ok::<Bytes, std::io::Error>(format_chunk(&update));
        }
        let mut ticker = tokio::time::interval(Duration::from_secs(HEARTBEAT_SECS));
        ticker.tick().await;
        loop {
            let notified = tokio::select! {
                event = rx.recv() => {
                    if matches!(event, Err(broadcast::error::RecvError::Closed)) { break; }
                    true
                }
                _ = ticker.tick() => false,
            };
            // Recover overlapping, reordered, lost and lagged notifications from the log.
            match log.updates_after(&game_id, &stream, cursor).await {
                Ok(updates) => for update in updates {
                    cursor = update.version as i64;
                    yield Ok(format_chunk(&update));
                },
                Err(error) => {
                    yield Err(std::io::Error::other(error));
                    break;
                }
            }
            if !notified { yield Ok(format_heartbeat()); }
        }
    };
    Response::builder()
        .status(209)
        .header("Content-Type", "application/http-history")
        .header("Cache-Control", "no-store")
        .header("Vary", "Subscribe, Parents, Version")
        .header("Heartbeats", HEARTBEAT_SECS.to_string())
        .body(Body::from_stream(body))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

#[cfg(test)]
mod tests {
    use super::super::tests::{migrated_pool, move_message};
    use super::*;
    use braid_chess::braid_http::{BraidClient, BraidRequest};

    #[tokio::test]
    async fn stable_versions_resume_with_stream_specific_parents() {
        let state = GameLogState::new(migrated_pool().await, None);
        state
            .put_event("g1", "moves", "alice", &move_message("fen1", 1), "v1", "0")
            .await
            .unwrap();
        let chat = braid_chess::ChessMessage::Chat(braid_chess::message::ChatPayload {
            player: "alice".into(),
            text: "hi".into(),
            timestamp_ms: 1,
        });
        state
            .put_event("g1", "chat", "alice", &chat, "chat1", "0")
            .await
            .unwrap();
        state
            .put_event("g1", "moves", "alice", &move_message("fen2", 2), "v2", "v1")
            .await
            .unwrap();
        let (all, _, _) = state.subscribe_from("g1", "moves", &[]).await.unwrap();
        assert_eq!(all.iter().map(|u| u.version).collect::<Vec<_>>(), [1, 3]);
        assert_eq!(all[1].parents, [1]);
        let (remaining, _, _) = state
            .subscribe_from("g1", "moves", &[Version::new("1")])
            .await
            .unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].version, 3);
        assert!(matches!(
            state
                .subscribe_from("g1", "moves", &[Version::new("2")])
                .await,
            Err(ResumeError::Gone)
        ));
        assert!(matches!(
            state
                .subscribe_from("g2", "moves", &[Version::new("1")])
                .await,
            Err(ResumeError::Gone)
        ));
        assert!(matches!(
            state
                .subscribe_from("g1", "moves", &[Version::new("99")])
                .await,
            Err(ResumeError::Gone)
        ));
    }

    #[tokio::test]
    async fn http_resume_recovers_lag_and_avoids_overlap_duplicates() {
        let state = Arc::new(GameLogState::new(migrated_pool().await, None));
        state
            .put_event("g1", "moves", "alice", &move_message("fen1", 1), "v1", "0")
            .await
            .unwrap();
        let log = state.clone();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let handler = axum::routing::get(move |headers: HeaderMap| {
            let log = log.clone();
            async move { stream_response(log, "g1", "moves", headers).await }
        });
        let router = axum::Router::new()
            .route("/", handler.clone())
            .route("/game/g1/moves", handler);
        let server = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        let client = BraidClient::new().unwrap();
        let mut subscription = client.subscribe(&url, BraidRequest::new()).await.unwrap();
        assert_eq!(
            subscription.next().await.unwrap().unwrap().version,
            [Version::new("1")]
        );
        drop(subscription);
        state
            .put_event("g1", "moves", "alice", &move_message("fen2", 2), "v2", "v1")
            .await
            .unwrap();
        let mut resumed = client
            .subscribe(&url, BraidRequest::new().with_parent(Version::new("1")))
            .await
            .unwrap();
        assert_eq!(
            resumed.next().await.unwrap().unwrap().version,
            [Version::new("2")]
        );
        state
            .put_event("g1", "moves", "alice", &move_message("fen3", 3), "v3", "v2")
            .await
            .unwrap();
        for _ in 0..100 {
            let _ = state
                .channel("g1", "moves")
                .await
                .send(BraidUpdate::snapshot(99, serde_json::Value::Null));
        }
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(3), resumed.next())
                .await
                .unwrap()
                .unwrap()
                .unwrap()
                .version,
            [Version::new("3")]
        );
        assert!(
            tokio::time::timeout(Duration::from_millis(100), resumed.next())
                .await
                .is_err()
        );
        let directory = tempfile::tempdir().unwrap();
        let journal =
            braid_chess::journal::Journal::open(directory.path(), "backend/g1/device").unwrap();
        let subscriber = braid_chess::ChessSubscriber::new(&url, "g1").unwrap();
        let (deliveries, _) = subscriber
            .subscribe_acknowledged(
                braid_chess::ChessResource::moves("g1"),
                vec![Version::new("2")],
            )
            .await
            .unwrap();
        let third = deliveries.recv().await.unwrap();
        assert_eq!(third.receipt.versions(), [Version::new("3")]);
        let fourth = move_message("fen4", 4);
        journal
            .enqueue(braid_chess::journal::PendingWrite {
                stream: "moves".into(),
                sender: "alice".into(),
                message: fourth.clone(),
                version: "v4".into(),
                parent: "v3".into(),
            })
            .unwrap();
        assert_eq!(
            state
                .put_event("g1", "moves", "alice", &fourth, "v4", "v3")
                .await
                .unwrap(),
            4
        );
        assert!(
            tokio::time::timeout(Duration::from_millis(100), deliveries.recv())
                .await
                .is_err()
        );
        journal
            .applied(
                "moves",
                third.receipt.versions().to_vec(),
                Some("fen3".into()),
            )
            .unwrap();
        third.receipt.acknowledge().unwrap();
        let unapplied = deliveries.recv().await.unwrap();
        assert_eq!(unapplied.receipt.versions(), [Version::new("4")]);
        drop(unapplied);
        drop(deliveries);
        drop(journal);
        let journal =
            braid_chess::journal::Journal::open(directory.path(), "backend/g1/device").unwrap();
        let pending = journal.pending().unwrap();
        assert_eq!(pending.len(), 1);
        // A response lost after commit must not create a second event on restart.
        assert_eq!(
            state
                .put_event(
                    "g1",
                    "moves",
                    "alice",
                    &pending[0].message,
                    &pending[0].version,
                    &pending[0].parent
                )
                .await
                .unwrap(),
            4
        );
        journal.confirm_write("v4").unwrap();
        assert!(journal.pending().unwrap().is_empty());
        let (restarted, _) = subscriber
            .subscribe_acknowledged(
                braid_chess::ChessResource::moves("g1"),
                journal.checkpoint("moves").unwrap().versions,
            )
            .await
            .unwrap();
        assert_eq!(
            restarted.recv().await.unwrap().receipt.versions(),
            [Version::new("4")]
        );
        drop(restarted);
        let response = reqwest::Client::new()
            .get(&url)
            .header("Subscribe", "true")
            .header("Parents", "\"999\"")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::GONE);
        server.abort();
    }
}
