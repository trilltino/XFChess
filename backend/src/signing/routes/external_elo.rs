//! External ELO linking routes for Lichess integration.
//!
//! Endpoints:
//! - POST /api/external-elo/link/start — start bio-nonce verification flow
//! - POST /api/external-elo/link/confirm — poll Lichess, verify, submit on-chain
//! - GET  /api/external-elo/status/{pubkey} — check current link status
//! - POST /api/external-elo/sync — force re-sync of Lichess ratings

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use solana_sdk::{pubkey::Pubkey, signature::Signer};
use std::str::FromStr;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tracing::{error, info, warn};

use crate::signing::{solana, AppState};

// ── Request / Response types ─────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct LinkStartReq {
    pub pubkey: String,
    pub username: String,
}

#[derive(Serialize)]
pub struct LinkStartResp {
    pub link_id: String,
    pub nonce: String,
    pub expires_at: u64,
}

#[derive(Deserialize)]
pub struct LinkConfirmReq {
    pub link_id: String,
}

#[derive(Serialize)]
pub struct LinkConfirmResp {
    pub tx_signature: String,
    pub lichess_username: String,
    pub blitz_rating: u32,
    pub rapid_rating: u32,
    pub bullet_rating: u32,
    pub seeded_elo: f64,
}

#[derive(Serialize)]
pub struct ExternalEloStatus {
    pub lichess: Option<LichessStatus>,
    pub on_chain_elo: f64,
    pub seeded_from_external: bool,
}

#[derive(Serialize)]
pub struct LichessStatus {
    pub username: String,
    pub verified: bool,
    pub blitz: u32,
    pub rapid: u32,
    pub bullet: u32,
    pub last_sync: i64,
}

#[derive(Deserialize)]
pub struct SyncReq {
    pub pubkey: String,
}

#[derive(Serialize)]
pub struct SyncResp {
    pub updated: bool,
    pub old_elo: f64,
    pub new_elo: f64,
}

/// Pending link stored in memory (MVP; could move to SQLite for persistence)
#[derive(Clone)]
struct PendingLink {
    pubkey: String,
    username: String,
    nonce: String,
    created_at: Instant,
}

use std::sync::{Arc, Mutex};
use once_cell::sync::Lazy;

static PENDING_LINKS: Lazy<Arc<Mutex<std::collections::HashMap<String, PendingLink>>>> =
    Lazy::new(|| Arc::new(Mutex::new(std::collections::HashMap::new())));

/// Creates the external-elo routes router.
pub fn external_elo_routes() -> Router<AppState> {
    Router::new()
        .route("/external-elo/link/start", post(link_start))
        .route("/external-elo/link/confirm", post(link_confirm))
        .route("/external-elo/status/{pubkey}", get(link_status))
        .route("/external-elo/sync", post(link_sync))
}

// ── Handlers ─────────────────────────────────────────────────────────────────

/// POST /api/external-elo/link/start
/// Generates a nonce and stores a pending link. Player must put nonce in Lichess bio.
async fn link_start(
    State(_state): State<AppState>,
    Json(req): Json<LinkStartReq>,
) -> Result<Json<LinkStartResp>, (StatusCode, String)> {
    // Validate pubkey
    let _pk = Pubkey::from_str(&req.pubkey)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid pubkey: {}", e)))?;

    // Validate username (Lichess: 2-30 chars, lowercase alphanumeric + hyphen/underscore)
    if req.username.len() < 2 || req.username.len() > 30 {
        return Err((StatusCode::BAD_REQUEST, "Username must be 2-30 characters".to_string()));
    }
    if !req.username.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_') {
        return Err((StatusCode::BAD_REQUEST, "Username contains invalid characters".to_string()));
    }

    let nonce = format!(
        "xfchess_link:{}:{}",
        req.pubkey,
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("System time should be after UNIX_EPOCH")
            .as_secs()
    );
    let link_id = format!("link_{}", uuid::Uuid::new_v4());
    let expires_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 300; // 5 minute expiry

    let pending = PendingLink {
        pubkey: req.pubkey.clone(),
        username: req.username.clone(),
        nonce: nonce.clone(),
        created_at: Instant::now(),
    };

    {
        let mut links = PENDING_LINKS.lock().expect("Pending links mutex should not be poisoned");
        links.insert(link_id.clone(), pending);
    }

    info!("[ExternalElo] Link start for {} -> Lichess user '{}' (nonce: {})", req.pubkey, req.username, nonce);

    Ok(Json(LinkStartResp {
        link_id,
        nonce,
        expires_at,
    }))
}

/// POST /api/external-elo/link/confirm
/// Polls Lichess API, verifies bio contains nonce, then submits on-chain tx.
async fn link_confirm(
    State(state): State<AppState>,
    Json(req): Json<LinkConfirmReq>,
) -> Result<Json<LinkConfirmResp>, (StatusCode, String)> {
    let pending = {
        let mut links = PENDING_LINKS.lock().expect("Pending links mutex should not be poisoned");
        links.remove(&req.link_id)
    };

    let pending = match pending {
        Some(p) => p,
        None => return Err((StatusCode::NOT_FOUND, "Link ID not found or expired".to_string())),
    };

    if pending.created_at.elapsed() > Duration::from_secs(300) {
        return Err((StatusCode::BAD_REQUEST, "Link expired".to_string()));
    }

    // Fetch Lichess user profile
    let lichess_url = format!("https://lichess.org/api/user/{}", pending.username);
    let client = reqwest::Client::new();
    let response = client
        .get(&lichess_url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("Lichess API error: {}", e)))?;

    if !response.status().is_success() {
        return Err((StatusCode::BAD_GATEWAY, format!("Lichess API returned {}", response.status())));
    }

    let lichess_data: serde_json::Value = response
        .json()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("Failed to parse Lichess response: {}", e)))?;

    // Verify bio contains nonce
    let bio = lichess_data
        .get("profile")
        .and_then(|p| p.get("bio"))
        .and_then(|b| b.as_str())
        .unwrap_or("");

    if !bio.contains(&pending.nonce) {
        return Err((
            StatusCode::PRECONDITION_FAILED,
            "Nonce not found in Lichess bio. Please paste the nonce into your profile bio and try again.".to_string(),
        ));
    }

    // Extract ratings
    let perfs = lichess_data.get("perfs").ok_or_else(|| {
        (StatusCode::BAD_GATEWAY, "Lichess profile missing 'perfs' field".to_string())
    })?;

    let extract_rating = |key: &str| -> u32 {
        perfs
            .get(key)
            .and_then(|p| p.get("rating"))
            .and_then(|r| r.as_u64())
            .unwrap_or(0) as u32
    };

    let blitz_rating = extract_rating("blitz");
    let rapid_rating = extract_rating("rapid");
    let bullet_rating = extract_rating("bullet");

    // Account age gate: reject accounts < 30 days old
    let created_at = lichess_data
        .get("createdAt")
        .and_then(|c| c.as_i64())
        .unwrap_or(0);
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    let thirty_days_ms = 30 * 24 * 60 * 60 * 1000;
    if now_ms - created_at < thirty_days_ms {
        return Err((
            StatusCode::FORBIDDEN,
            "Lichess account must be at least 30 days old to link.".to_string(),
        ));
    }

    // Games played gate: require at least 20 rated games in at least one time control
    let games_count = |key: &str| -> u32 {
        perfs
            .get(key)
            .and_then(|p| p.get("games"))
            .and_then(|g| g.as_u64())
            .unwrap_or(0) as u32
    };
    let total_games = games_count("blitz") + games_count("rapid") + games_count("bullet");
    if total_games < 20 {
        return Err((
            StatusCode::FORBIDDEN,
            "Lichess account must have at least 20 rated games to link.".to_string(),
        ));
    }

    // Build and submit on-chain transaction
    let player_pk = Pubkey::from_str(&pending.pubkey)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid pubkey: {}", e)))?;
    let program_id = state.program_id;
    let link_authority = &state.link_authority;

    let ix = solana::link_external_elo_ix(
        &program_id,
        &link_authority.pubkey(),
        &player_pk,
        &pending.username,
        blitz_rating * 100, // Convert to centiscale
        rapid_rating * 100,
        bullet_rating * 100,
    );

    let rpc = solana::make_rpc(&state.solana_rpc_url);
    let tx_sig = match solana::sign_and_submit(&rpc, link_authority, &[ix]) {
        Ok(sig) => sig.to_string(),
        Err(e) => {
            error!("[ExternalElo] On-chain submission failed for {}: {}", pending.pubkey, e);
            return Err((StatusCode::BAD_GATEWAY, format!("On-chain submission failed: {}", e)));
        }
    };

    info!(
        "[ExternalElo] Linked {} -> Lichess '{}' (Blitz: {}, Rapid: {}, Bullet: {}) tx: {}",
        pending.pubkey, pending.username, blitz_rating, rapid_rating, bullet_rating, tx_sig
    );

    // Store in backend SQLite for future reference
    let pool = state.store.pool();
    if let Err(e) = store_link_in_db(
        pool,
        &pending.pubkey,
        &pending.username,
        blitz_rating,
        rapid_rating,
        bullet_rating,
        &tx_sig,
    ).await {
        warn!("[ExternalElo] Failed to store link in DB: {}", e);
    }

    // Invalidate ELO cache so next fetch reflects the seeded value
    state.elo_cache.invalidate(&pending.pubkey);

    let seeded_elo = if blitz_rating > rapid_rating + 500 {
        blitz_rating as f64
    } else {
        rapid_rating as f64
    };

    Ok(Json(LinkConfirmResp {
        tx_signature: tx_sig,
        lichess_username: pending.username,
        blitz_rating,
        rapid_rating,
        bullet_rating,
        seeded_elo,
    }))
}

/// GET /api/external-elo/status/{pubkey}
async fn link_status(
    State(state): State<AppState>,
    Path(pubkey): Path<String>,
) -> Result<Json<ExternalEloStatus>, StatusCode> {
    // Fetch on-chain profile via EloCache
    let on_chain = match state.elo_cache.get_elo(&pubkey).await {
        Ok(data) => data,
        Err(_) => {
            return Ok(Json(ExternalEloStatus {
                lichess: None,
                on_chain_elo: 1200.0,
                seeded_from_external: false,
            }));
        }
    };

    // Try to fetch from backend DB for additional details
    let pool = state.store.pool();
    let db_link = fetch_link_from_db(pool, &pubkey).await.ok();

    let lichess = if on_chain.lichess_verified || db_link.is_some() {
        Some(LichessStatus {
            username: db_link.as_ref().map(|l| l.username.clone()).unwrap_or_default(),
            verified: on_chain.lichess_verified,
            blitz: db_link.as_ref().map(|l| l.blitz_rating).unwrap_or(0),
            rapid: db_link.as_ref().map(|l| l.rapid_rating).unwrap_or(0),
            bullet: db_link.as_ref().map(|l| l.bullet_rating).unwrap_or(0),
            last_sync: on_chain.lichess_last_sync,
        })
    } else {
        None
    };

    Ok(Json(ExternalEloStatus {
        lichess,
        on_chain_elo: on_chain.elo_rating / 100.0,
        seeded_from_external: on_chain.seeded_from_external,
    }))
}

/// POST /api/external-elo/sync
/// Forces a re-sync of Lichess ratings for an already-linked account.
async fn link_sync(
    State(state): State<AppState>,
    Json(req): Json<SyncReq>,
) -> Result<Json<SyncResp>, (StatusCode, String)> {
    let player_pk = Pubkey::from_str(&req.pubkey)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid pubkey: {}", e)))?;

    // Fetch current on-chain ELO
    let old_elo = state
        .elo_cache
        .get_elo(&req.pubkey)
        .await
        .map(|e| e.elo_rating / 100.0)
        .unwrap_or(1200.0);

    // Fetch from backend DB to get the username
    let pool = state.store.pool();
    let db_link = fetch_link_from_db(pool.clone(), &req.pubkey)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, format!("No linked Lichess account found: {}", e)))?;

    // Poll Lichess API
    let lichess_url = format!("https://lichess.org/api/user/{}", db_link.username);
    let client = reqwest::Client::new();
    let response = client
        .get(&lichess_url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("Lichess API error: {}", e)))?;

    if !response.status().is_success() {
        return Err((StatusCode::BAD_GATEWAY, format!("Lichess API returned {}", response.status())));
    }

    let lichess_data: serde_json::Value = response
        .json()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("Failed to parse Lichess response: {}", e)))?;

    let perfs = lichess_data.get("perfs").unwrap_or(&serde_json::Value::Null);
    let extract_rating = |key: &str| -> u32 {
        perfs
            .get(key)
            .and_then(|p| p.get("rating"))
            .and_then(|r| r.as_u64())
            .unwrap_or(0) as u32
    };

    let blitz_rating = extract_rating("blitz");
    let rapid_rating = extract_rating("rapid");
    let bullet_rating = extract_rating("bullet");

    // Submit on-chain update (re-link to refresh ratings)
    let program_id = state.program_id;
    let link_authority = &state.link_authority;

    let ix = solana::link_external_elo_ix(
        &program_id,
        &link_authority.pubkey(),
        &player_pk,
        &db_link.username,
        blitz_rating * 100,
        rapid_rating * 100,
        bullet_rating * 100,
    );

    let rpc = solana::make_rpc(&state.solana_rpc_url);
    let tx_sig = match solana::sign_and_submit(&rpc, link_authority, &[ix]) {
        Ok(sig) => sig.to_string(),
        Err(e) => {
            error!("[ExternalElo] Sync on-chain submission failed for {}: {}", req.pubkey, e);
            return Err((StatusCode::BAD_GATEWAY, format!("On-chain submission failed: {}", e)));
        }
    };

    // Update DB
    if let Err(e) = store_link_in_db(
        pool,
        &req.pubkey,
        &db_link.username,
        blitz_rating,
        rapid_rating,
        bullet_rating,
        &tx_sig,
    ).await {
        warn!("[ExternalElo] Failed to update link in DB during sync: {}", e);
    }

    state.elo_cache.invalidate(&req.pubkey);

    let new_elo = state
        .elo_cache
        .get_elo(&req.pubkey)
        .await
        .map(|e| e.elo_rating / 100.0)
        .unwrap_or(old_elo);

    info!(
        "[ExternalElo] Synced {} -> Lichess '{}' updated (Blitz: {}, Rapid: {}, Bullet: {}) tx: {}",
        req.pubkey, db_link.username, blitz_rating, rapid_rating, bullet_rating, tx_sig
    );

    Ok(Json(SyncResp {
        updated: (new_elo - old_elo).abs() > 0.01,
        old_elo,
        new_elo,
    }))
}

// ── DB helpers ───────────────────────────────────────────────────────────────

#[derive(sqlx::FromRow)]
struct DbExternalEloLink {
    username: String,
    blitz_rating: u32,
    rapid_rating: u32,
    bullet_rating: u32,
}

async fn store_link_in_db(
    pool: sqlx::SqlitePool,
    pubkey: &str,
    username: &str,
    blitz: u32,
    rapid: u32,
    bullet: u32,
    tx_sig: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO external_elo_links (
            pubkey, platform, username, verified, blitz_rating, rapid_rating, bullet_rating,
            linked_at, last_sync_at, on_chain_tx
        ) VALUES (?, 'lichess', ?, 1, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(pubkey, platform) DO UPDATE SET
            username = excluded.username,
            verified = excluded.verified,
            blitz_rating = excluded.blitz_rating,
            rapid_rating = excluded.rapid_rating,
            bullet_rating = excluded.bullet_rating,
            last_sync_at = excluded.last_sync_at,
            on_chain_tx = excluded.on_chain_tx
        "#
    )
    .bind(pubkey)
    .bind(username)
    .bind(blitz as i64)
    .bind(rapid as i64)
    .bind(bullet as i64)
    .bind(chrono::Utc::now().timestamp())
    .bind(chrono::Utc::now().timestamp())
    .bind(tx_sig)
    .execute(&pool)
    .await?;

    Ok(())
}

async fn fetch_link_from_db(
    pool: sqlx::SqlitePool,
    pubkey: &str,
) -> Result<DbExternalEloLink, sqlx::Error> {
    sqlx::query_as::<_, DbExternalEloLink>(
        r#"
        SELECT username, blitz_rating, rapid_rating, bullet_rating
        FROM external_elo_links
        WHERE pubkey = ? AND platform = 'lichess'
        "#
    )
    .bind(pubkey)
    .fetch_one(&pool)
    .await
}
