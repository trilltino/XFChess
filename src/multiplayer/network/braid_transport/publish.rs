use super::{open_journal, SharedStreamHeads};
use braid_chess::{
    journal::{Journal, PendingWrite},
    ChessMessage,
};
use serde::{Deserialize, Serialize};
use std::{fs::OpenOptions, time::Duration};
use tracing::{debug, warn};

#[derive(Serialize)]
struct GameEventReq<'a> {
    #[serde(rename = "player_pubkey")]
    sender_identity: &'a str,
    session_token: &'a str,
    message: &'a ChessMessage,
    content_version: &'a str,
    content_parent: &'a str,
    device_id: &'a str,
}

#[derive(Deserialize)]
struct ParentMismatchResp {
    expected_parent: String,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn publish(
    base_url: String,
    game_id: String,
    stream: &'static str,
    sender_identity: String,
    session_token: String,
    message: ChessMessage,
    content_version: String,
    heads: SharedStreamHeads,
) {
    let diagnostic_game = game_id.clone();
    let result = (|| {
        let journal = open_journal(&base_url, &game_id)?;
        let parent = heads
            .lock()
            .map_err(|_| std::io::Error::other("stream heads poisoned"))?
            .head(stream);
        journal.enqueue(PendingWrite {
            stream: stream.into(),
            sender: sender_identity.clone(),
            message,
            version: content_version.clone(),
            parent,
        })?;
        if let Ok(mut heads) = heads.lock() {
            heads.self_published.insert(content_version);
        }
        crate::core::net_task::spawn_detached("publish", move || {
            if let Err(error) = flush(
                &journal,
                &base_url,
                &game_id,
                stream,
                &sender_identity,
                &session_token,
                &heads,
            ) {
                warn!("[braid] pending events retained for game {game_id}: {error}");
            }
        });
        Ok::<_, std::io::Error>(())
    })();
    if let Err(error) = result {
        warn!("[braid] could not save pending event for game {diagnostic_game}: {error}");
    }
}

pub(crate) fn retry_pending(
    base_url: String,
    game_id: String,
    sender: String,
    session_token: String,
    heads: SharedStreamHeads,
) {
    let Ok(journal) = open_journal(&base_url, &game_id) else {
        return;
    };
    let Ok(pending) = journal.pending() else {
        return;
    };
    for stream in ["moves", "chat"] {
        if !pending
            .iter()
            .any(|entry| entry.sender == sender && entry.stream == stream)
        {
            continue;
        }
        let (journal, base_url, game_id, sender, session_token, heads) = (
            journal.clone(),
            base_url.clone(),
            game_id.clone(),
            sender.clone(),
            session_token.clone(),
            heads.clone(),
        );
        crate::core::net_task::spawn_detached("publish", move || {
            if let Err(error) = flush(
                &journal,
                &base_url,
                &game_id,
                stream,
                &sender,
                &session_token,
                &heads,
            ) {
                warn!("[braid] pending writes retained for game {game_id}: {error}");
            }
        });
    }
}

fn flush(
    journal: &Journal,
    base_url: &str,
    game_id: &str,
    stream: &str,
    sender: &str,
    session_token: &str,
    heads: &SharedStreamHeads,
) -> Result<(), String> {
    let worker = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(journal.directory().join(format!("publisher.{stream}.lock")))
        .map_err(|e| e.to_string())?;
    match worker.try_lock() {
        Ok(()) => {}
        Err(std::fs::TryLockError::WouldBlock) => return Ok(()),
        Err(std::fs::TryLockError::Error(error)) => return Err(error.to_string()),
    }
    let client = crate::multiplayer::network::vps::client_fast()?;
    while let Some(mut write) = journal
        .pending()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|entry| entry.sender == sender && entry.stream == stream)
    {
        if let Ok(mut heads) = heads.lock() {
            heads.self_published.insert(write.version.clone());
        }
        let url = format!(
            "{}/game/{game_id}/{}",
            base_url.trim_end_matches('/'),
            write.stream
        );
        let mut failures = 0;
        loop {
            let body = GameEventReq {
                sender_identity: sender,
                session_token,
                message: &write.message,
                content_version: &write.version,
                content_parent: &write.parent,
                device_id: crate::multiplayer::network::device_id::device_id(),
            };
            match client.put(&url).json(&body).send() {
                Ok(response) if response.status() == reqwest::StatusCode::OK => {
                    journal
                        .confirm_write(&write.version)
                        .map_err(|e| e.to_string())?;
                    if let Ok(mut heads) = heads.lock() {
                        heads.set_head(&write.stream, write.version.clone());
                    }
                    debug!("[braid] saved {game_id}/{}", write.stream);
                    break;
                }
                Ok(response) if response.status() == reqwest::StatusCode::CONFLICT => {
                    let text = response.text().map_err(|e| e.to_string())?;
                    if crate::multiplayer::network::device_id::is_seat_superseded_body(&text) {
                        if let Ok(id) = game_id.parse() {
                            crate::multiplayer::network::device_id::mark_seat_superseded(id);
                        }
                        return Err("seat superseded".into());
                    }
                    let conflict: ParentMismatchResp = serde_json::from_str(&text)
                        .map_err(|e| format!("invalid conflict: {e}"))?;
                    journal
                        .reparent(&write.version, &conflict.expected_parent)
                        .map_err(|e| e.to_string())?;
                    write.parent = conflict.expected_parent;
                }
                Ok(response)
                    if response.status().is_server_error()
                        || response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS => {}
                Ok(response) => {
                    return Err(format!(
                        "backend rejected event (HTTP {})",
                        response.status()
                    ))
                }
                Err(error) => warn!("[braid] publish interrupted for game {game_id}: {error}"),
            }
            failures += 1;
            if failures >= 5 {
                return Err("publish retry budget exhausted".into());
            }
            std::thread::sleep(Duration::from_secs(1 << (failures - 1)));
        }
    }
    Ok(())
}
