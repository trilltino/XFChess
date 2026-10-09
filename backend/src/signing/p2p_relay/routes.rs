use axum::{
    extract::{Json, Query, State},
    http::StatusCode,
    response::Json as AxumJson,
    routing::{get, post},
    Router,
};
use chrono::Utc;
use serde::Deserialize;

use crate::signing::AppState;

use super::types::{
    AcceptJoinReq, ActiveGame, AnnounceGameRequest, AnnounceGameResponse, GameListing, GameStatus,
    HeartbeatRequest, JoinGameRequest, JoinGameResponse, LeaveGameRequest, PollMessagesRequest,
    PollMessagesResponse, SendMessageRequest, LOBBY_TTL_SECS,
};

#[derive(Debug, Default, Deserialize)]
pub struct LobbyFilter {
    pub time_min: Option<u32>,
    pub time_max: Option<u32>,
    pub stake_min: Option<f64>,
    pub stake_max: Option<f64>,
    pub elo_min: Option<u16>,
    pub elo_max: Option<u16>,
    pub sort: Option<String>,
}

pub fn p2p_routes() -> Router<AppState> {
    Router::new()
        .route("/p2p/announce", post(announce_game))
        .route("/p2p/games", get(list_games))
        .route("/p2p/join", post(join_game))
        .route("/p2p/accept", post(accept_join))
        .route("/p2p/leave", post(leave_game))
        .route("/p2p/heartbeat", post(heartbeat_game))
        .route("/p2p/message", post(send_message))
        .route("/p2p/poll", post(poll_messages))
        .route("/region", get(get_region))
}

pub async fn announce_game(
    State(state): State<AppState>,
    Json(req): Json<AnnounceGameRequest>,
) -> Result<AxumJson<AnnounceGameResponse>, StatusCode> {
    let password_hash = req.password.as_deref().map(|p| {
        use argon2::{
            password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
            Argon2,
        };
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default()
            .hash_password(p.as_bytes(), &salt)
            .map(|h| h.to_string())
            .unwrap_or_default()
    });

    let game_id = req.game_id.clone();
    let host_node_id = req.host_node_id.clone();
    let saved = {
        let mut games = state
            .p2p_relay
            .write()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        match games.get_mut(&game_id) {
            // Re-announce by the same host (e.g. after a backend restart or a
            // client reconnect): refresh the room but keep its joiner, status
            // and undelivered messages instead of resetting the handshake.
            Some(existing)
                if existing.announcement.host_node_id == host_node_id
                    && existing.announcement.status != GameStatus::Finished =>
            {
                existing.last_activity = Utc::now();
                existing.clone()
            }
            // Someone else's live room: never overwrite it.
            Some(existing) if existing.announcement.status != GameStatus::Finished => {
                tracing::warn!(
                    "[p2p-relay] rejected announce for {} by {}: room belongs to another host",
                    game_id,
                    host_node_id
                );
                return Ok(AxumJson(AnnounceGameResponse { success: false }));
            }
            _ => {
                let active_game = ActiveGame {
                    announcement: super::types::P2PGameAnnouncement {
                        game_id: req.game_id,
                        host_node_id: req.host_node_id,
                        display_name: req.display_name,
                        stake_amount: req.stake_amount,
                        game_type: req.game_type,
                        base_time_seconds: req.base_time_seconds,
                        increment_seconds: req.increment_seconds,
                        created_at: Utc::now(),
                        status: GameStatus::Open,
                        username: req.username,
                        elo: req.elo,
                        region: req.region,
                        password_hash,
                    },
                    joiner_node_id: None,
                    host_messages: Vec::new(),
                    joiner_messages: Vec::new(),
                    last_activity: Utc::now(),
                    pending_invites: Vec::new(),
                };
                games.insert(game_id.clone(), active_game.clone());
                active_game
            }
        }
    };
    state.p2p_relay_store.save(&saved).await;

    tracing::info!("P2P game announced: {} by {}", game_id, host_node_id);

    Ok(AxumJson(AnnounceGameResponse { success: true }))
}

pub async fn list_games(
    State(state): State<AppState>,
    Query(filter): Query<LobbyFilter>,
) -> Result<AxumJson<Vec<GameListing>>, StatusCode> {
    let relay_state = state.p2p_relay.clone();
    let games = relay_state
        .read()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let now = Utc::now();

    // Open (joinable) AND Connecting/InProgress (already underway) are all
    // listed — spectators need to see live games, not just open lobbies.
    // Only Finished games are dropped. Client-side, the join-lobby screen
    // already disables its Join button once `players_joined >= capacity`
    // (independent of this status field), and the spectator popup filters
    // to exactly that condition — so a non-Open game showing up here can't
    // be joined twice, only watched.
    let mut listings: Vec<GameListing> = games
        .values()
        .filter(|g| g.announcement.status != GameStatus::Finished)
        .filter(|g| {
            // time_control filter
            let t = g.announcement.base_time_seconds;
            filter.time_min.map_or(true, |mn| t >= mn) && filter.time_max.map_or(true, |mx| t <= mx)
        })
        .filter(|g| {
            let s = g.announcement.stake_amount;
            filter.stake_min.map_or(true, |mn| s >= mn)
                && filter.stake_max.map_or(true, |mx| s <= mx)
        })
        .filter(|g| {
            if filter.elo_min.is_none() && filter.elo_max.is_none() {
                return true;
            }
            let elo = g.announcement.elo.unwrap_or(1200);
            filter.elo_min.map_or(true, |mn| elo >= mn)
                && filter.elo_max.map_or(true, |mx| elo <= mx)
        })
        .map(|g| {
            let elapsed = now.signed_duration_since(g.last_activity).num_seconds();
            let ttl_seconds = (LOBBY_TTL_SECS - elapsed).max(0);
            GameListing {
                game_id: g.announcement.game_id.clone(),
                display_name: g.announcement.display_name.clone(),
                stake_amount: g.announcement.stake_amount,
                game_type: g.announcement.game_type.clone(),
                base_time_seconds: g.announcement.base_time_seconds,
                increment_seconds: g.announcement.increment_seconds,
                status: g.announcement.status.clone(),
                username: g.announcement.username.clone(),
                elo: g.announcement.elo,
                region: g.announcement.region.clone(),
                capacity: 2,
                players_joined: if g.joiner_node_id.is_some() { 2 } else { 1 },
                ttl_seconds,
                is_private: g.announcement.password_hash.is_some(),
            }
        })
        .collect();

    // Sort
    match filter.sort.as_deref().unwrap_or("newest") {
        "elo_asc" => listings.sort_by_key(|l| l.elo.unwrap_or(0)),
        "elo_desc" => listings.sort_by(|a, b| b.elo.unwrap_or(0).cmp(&a.elo.unwrap_or(0))),
        "stake_asc" => listings.sort_by(|a, b| {
            a.stake_amount
                .partial_cmp(&b.stake_amount)
                .unwrap_or(std::cmp::Ordering::Equal)
        }),
        "stake_desc" => listings.sort_by(|a, b| {
            b.stake_amount
                .partial_cmp(&a.stake_amount)
                .unwrap_or(std::cmp::Ordering::Equal)
        }),
        "time_asc" => listings.sort_by_key(|l| l.base_time_seconds),
        _ => listings.sort_by(|a, b| b.ttl_seconds.cmp(&a.ttl_seconds)), // newest first
    }

    Ok(AxumJson(listings))
}

pub async fn join_game(
    State(state): State<AppState>,
    Json(req): Json<JoinGameRequest>,
) -> Result<AxumJson<JoinGameResponse>, StatusCode> {
    let rejected = || {
        Ok(AxumJson(JoinGameResponse {
            success: false,
            host_node_id: None,
        }))
    };
    let (saved, host_node_id) = {
        let mut games = state
            .p2p_relay
            .write()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let Some(game) = games.get_mut(&req.game_id) else {
            return rejected();
        };

        // A retried join from the same joiner (lost response, restart) is
        // idempotent; any other join only succeeds against an Open room.
        let is_retry = game.joiner_node_id.as_deref() == Some(req.joiner_node_id.as_str())
            && game.announcement.status != GameStatus::Finished;
        if !is_retry && game.announcement.status != GameStatus::Open {
            return rejected();
        }

        // Password check for private rooms
        if let Some(ref hash) = game.announcement.password_hash.clone() {
            use argon2::{password_hash::PasswordHash, password_hash::PasswordVerifier, Argon2};
            let provided = req.password.as_deref().unwrap_or("");
            let verified = PasswordHash::new(hash)
                .ok()
                .map(|parsed| {
                    Argon2::default()
                        .verify_password(provided.as_bytes(), &parsed)
                        .is_ok()
                })
                .unwrap_or(false);
            if !verified {
                tracing::warn!("Wrong password for game {}", req.game_id);
                return rejected();
            }
        }

        if !is_retry {
            game.joiner_node_id = Some(req.joiner_node_id.clone());
            game.announcement.status = GameStatus::Connecting;
        }
        game.last_activity = Utc::now();
        (game.clone(), game.announcement.host_node_id.clone())
    };
    state.p2p_relay_store.save(&saved).await;

    tracing::info!(
        "P2P join request: game={}, joiner={}",
        req.game_id,
        req.joiner_node_id
    );

    Ok(AxumJson(JoinGameResponse {
        success: true,
        host_node_id: Some(host_node_id),
    }))
}

pub async fn accept_join(
    State(state): State<AppState>,
    Json(req): Json<AcceptJoinReq>,
) -> Result<AxumJson<AnnounceGameResponse>, StatusCode> {
    let (saved, joiner_node_id) = {
        let mut games = state
            .p2p_relay
            .write()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let Some(game) = games.get_mut(&req.game_id) else {
            return Ok(AxumJson(AnnounceGameResponse { success: false }));
        };

        // Verify caller is the host.
        if game.announcement.host_node_id != req.host_node_id {
            return Ok(AxumJson(AnnounceGameResponse { success: false }));
        }

        // Refuse if no joiner has arrived yet — prevents accept racing ahead of join.
        let Some(joiner_node_id) = game.joiner_node_id.clone() else {
            return Ok(AxumJson(AnnounceGameResponse { success: false }));
        };

        game.announcement.status = GameStatus::InProgress;
        game.last_activity = chrono::Utc::now();
        (game.clone(), joiner_node_id)
    };
    state.p2p_relay_store.save(&saved).await;
    tracing::info!("P2P game {} started", req.game_id);

    state
        .game_log
        .register_casual_identities(&req.game_id, &req.host_node_id, &joiner_node_id)
        .await
        .map_err(|e| {
            tracing::error!(
                "Failed to persist participants for casual game {}: {e}",
                req.game_id
            );
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .then_some(())
        .ok_or(StatusCode::CONFLICT)?;

    Ok(AxumJson(AnnounceGameResponse { success: true }))
}

pub async fn leave_game(
    State(state): State<AppState>,
    Json(req): Json<LeaveGameRequest>,
) -> Result<AxumJson<AnnounceGameResponse>, StatusCode> {
    let saved = {
        let mut games = state
            .p2p_relay
            .write()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let Some(game) = games.get_mut(&req.game_id) else {
            return Ok(AxumJson(AnnounceGameResponse { success: true }));
        };
        let money_linked =
            game.announcement.stake_amount > 0.0 || game.announcement.game_type == "solana_wager";
        if money_linked {
            tracing::warn!(
                "[p2p-relay] ignored unsigned leave for money-linked game {} from {}; relay state is advisory, cancel/refund must be proven on-chain",
                req.game_id,
                req.node_id
            );
            return Ok(AxumJson(AnnounceGameResponse { success: false }));
        }
        // A relay leave can be caused by a closed window or a failed
        // transport. Once play began it cannot decide the chess result or
        // reopen the room for a third player.
        if game.announcement.status == GameStatus::InProgress {
            return Ok(AxumJson(AnnounceGameResponse { success: true }));
        }
        if game.announcement.host_node_id == req.node_id {
            // Host left - remove game
            game.announcement.status = GameStatus::Finished;
            tracing::info!("P2P game {} ended (host left)", req.game_id);
        } else if game.joiner_node_id.as_ref() == Some(&req.node_id) {
            // Joiner left
            game.joiner_node_id = None;
            game.announcement.status = GameStatus::Open;
            tracing::info!("P2P game {} open again (joiner left)", req.game_id);
        } else {
            return Ok(AxumJson(AnnounceGameResponse { success: true }));
        }
        game.clone()
    };
    state.p2p_relay_store.save(&saved).await;

    Ok(AxumJson(AnnounceGameResponse { success: true }))
}

fn verify_p2p_message_signature(req: &SendMessageRequest) -> bool {
    use solana_sdk::{pubkey::Pubkey, signature::Signature};
    use std::str::FromStr;

    let Ok(pk) = Pubkey::from_str(&req.from_node_id) else {
        return false;
    };
    let Ok(sig_bytes): Result<[u8; 64], _> = req.signature.as_slice().try_into() else {
        return false;
    };
    let sig = Signature::from(sig_bytes);
    let signable = format!("{}:{}:{}", req.game_id, req.from_node_id, req.message);
    sig.verify(pk.as_ref(), signable.as_bytes())
}

pub async fn send_message(
    State(state): State<AppState>,
    Json(req): Json<SendMessageRequest>,
) -> Result<AxumJson<AnnounceGameResponse>, StatusCode> {
    if !verify_p2p_message_signature(&req) {
        tracing::warn!(
            "[p2p-relay] rejected send_message for game {} — signature does not match claimed from_node_id {}",
            req.game_id, req.from_node_id
        );
        return Ok(AxumJson(AnnounceGameResponse { success: false }));
    }

    let saved = {
        let mut games = state
            .p2p_relay
            .write()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let Some(game) = games.get_mut(&req.game_id) else {
            return Ok(AxumJson(AnnounceGameResponse { success: false }));
        };

        game.last_activity = chrono::Utc::now();

        if game.announcement.host_node_id == req.from_node_id {
            // Message from host to joiner
            game.host_messages.push(req.message);
        } else if game.joiner_node_id.as_ref() == Some(&req.from_node_id) {
            // Message from joiner to host
            game.joiner_messages.push(req.message);
        } else {
            return Ok(AxumJson(AnnounceGameResponse { success: false }));
        }
        game.clone()
    };
    state.p2p_relay_store.save(&saved).await;

    Ok(AxumJson(AnnounceGameResponse { success: true }))
}

/// Messages after `since_index`. An index past the end (a client that kept
/// its cursor across a backend restart) yields an empty page and moves the
/// cursor to where the mailbox actually ends, instead of panicking on an
/// out-of-range slice.
fn page_from(messages: &[String], since_index: usize) -> (Vec<String>, usize) {
    match messages.get(since_index..) {
        Some(page) => (page.to_vec(), since_index + page.len()),
        None => (Vec::new(), messages.len()),
    }
}

pub async fn poll_messages(
    State(state): State<AppState>,
    Json(req): Json<PollMessagesRequest>,
) -> Result<AxumJson<PollMessagesResponse>, StatusCode> {
    let relay_state = state.p2p_relay.clone();
    let games = relay_state
        .read()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let Some(game) = games.get(&req.game_id) else {
        return Ok(AxumJson(PollMessagesResponse {
            messages: vec![],
            next_index: req.since_index,
        }));
    };

    let (messages, next_index) = if game.announcement.host_node_id == req.node_id {
        // Host polls joiner messages
        page_from(&game.joiner_messages, req.since_index)
    } else if game.joiner_node_id.as_ref() == Some(&req.node_id) {
        // Joiner polls host messages
        page_from(&game.host_messages, req.since_index)
    } else {
        (vec![], req.since_index)
    };

    Ok(AxumJson(PollMessagesResponse {
        messages,
        next_index,
    }))
}

pub async fn heartbeat_game(
    State(state): State<AppState>,
    Json(req): Json<HeartbeatRequest>,
) -> Result<AxumJson<AnnounceGameResponse>, StatusCode> {
    let saved = {
        let mut games = state
            .p2p_relay
            .write()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        match games.get_mut(&req.game_id) {
            Some(game) if game.announcement.host_node_id == req.host_node_id => {
                game.last_activity = Utc::now();
                game.clone()
            }
            _ => return Ok(AxumJson(AnnounceGameResponse { success: false })),
        }
    };
    state.p2p_relay_store.save(&saved).await;
    Ok(AxumJson(AnnounceGameResponse { success: true }))
}

pub async fn get_region() -> AxumJson<serde_json::Value> {
    let region = std::env::var("XFCHESS_REGION").unwrap_or_else(|_| "unknown".to_string());
    let label = match region.as_str() {
        "eu-central" | "eu" => "EU (Frankfurt)",
        "us-east" | "us" => "US East (New York)",
        "us-west" => "US West (Los Angeles)",
        "ap-southeast" => "Asia (Singapore)",
        "ap-northeast" => "Asia (Tokyo)",
        _ => "Unknown Region",
    };
    AxumJson(serde_json::json!({ "region": region, "label": label }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::signer::{keypair::Keypair, Signer};

    fn signed_request(
        signer: &Keypair,
        claimed_from: &str,
        game_id: &str,
        message: &str,
    ) -> SendMessageRequest {
        let signable = format!("{}:{}:{}", game_id, claimed_from, message);
        let signature = signer.sign_message(signable.as_bytes()).as_ref().to_vec();
        SendMessageRequest {
            game_id: game_id.to_string(),
            from_node_id: claimed_from.to_string(),
            message: message.to_string(),
            signature,
        }
    }

    #[test]
    fn poll_cursor_past_end_after_restart_does_not_panic() {
        let msgs = vec!["a".to_string(), "b".to_string()];
        assert_eq!(page_from(&msgs, 0), (msgs.clone(), 2));
        assert_eq!(page_from(&msgs, 1), (vec!["b".to_string()], 2));
        assert_eq!(page_from(&msgs, 2), (vec![], 2));
        // A cursor from before a restart that lost later messages.
        assert_eq!(page_from(&msgs, 9), (vec![], 2));
        assert_eq!(page_from(&[], 5), (vec![], 0));
    }

    #[test]
    fn genuine_signature_verifies() {
        let real_owner = Keypair::new();
        let req = signed_request(
            &real_owner,
            &real_owner.pubkey().to_string(),
            "g1",
            "JOIN_ACK:x|y|1200",
        );
        assert!(verify_p2p_message_signature(&req));
    }

    #[test]
    fn forged_claimed_identity_is_rejected() {
        let attacker = Keypair::new();
        let victim = Keypair::new();
        // Attacker signs with their OWN key but claims to be the victim's
        // node_id in the payload — signature won't match the claimed identity.
        let req = signed_request(
            &attacker,
            &victim.pubkey().to_string(),
            "g1",
            "JOIN_ACK:victim|hijacked|9999",
        );
        assert!(!verify_p2p_message_signature(&req));
    }

    #[test]
    fn tampered_message_after_signing_is_rejected() {
        let signer = Keypair::new();
        let mut req = signed_request(
            &signer,
            &signer.pubkey().to_string(),
            "g1",
            "JOIN_ACK:x|y|1200",
        );
        req.message = "JOIN_ACK:x|y|9999".to_string(); // changed after signing
        assert!(!verify_p2p_message_signature(&req));
    }

    #[test]
    fn malformed_signature_bytes_are_rejected_not_panicking() {
        let signer = Keypair::new();
        let mut req = signed_request(&signer, &signer.pubkey().to_string(), "g1", "hi");
        req.signature = vec![1, 2, 3]; // wrong length, not a valid 64-byte signature
        assert!(!verify_p2p_message_signature(&req));
    }

    #[test]
    fn non_pubkey_from_node_id_is_rejected_not_panicking() {
        let signer = Keypair::new();
        let req = signed_request(&signer, "not-a-valid-base58-pubkey", "g1", "hi");
        assert!(!verify_p2p_message_signature(&req));
    }
}
