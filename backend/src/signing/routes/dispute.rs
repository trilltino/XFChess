//! Dispute management routes.
//!
//! POST /dispute/notify      — player calls after submitting on-chain tx
//! GET  /dispute/:game_id    — returns current dispute status
//! POST /admin/dispute/resolve — moderator resolves a dispute (admin-only)

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signer;
use std::env;
use std::str::FromStr;
use tracing::{error, info, warn};

use crate::db::repository::DisputeRepository;
use crate::signing::AppState;

const DISPUTE_NOTIFY_EMAIL: &str = "isicheivalentine@gmail.com";
const FROM_EMAIL: &str = "noreply@xfchess.com";

// ── Route builder ─────────────────────────────────────────────────────────────

pub fn dispute_routes() -> Router<AppState> {
    Router::new()
        .route("/dispute/notify", post(notify_dispute))
        .route("/dispute/{game_id}", get(get_dispute_status))
}

pub fn admin_dispute_routes() -> Router<AppState> {
    Router::new()
        .route("/admin/dispute/resolve", post(resolve_dispute))
}

// ── Request / Response types ──────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct NotifyDisputeReq {
    pub game_id: i64,
    pub challenger_wallet: String,
    pub reason: String,
    pub tx_signature: String,
}

#[derive(Serialize)]
pub struct NotifyDisputeResp {
    pub ok: bool,
    pub case_id: String,
}

#[derive(Deserialize)]
pub struct ResolveDisputeReq {
    pub game_id: i64,
    /// WHITE_WINS | BLACK_WINS | DRAW | DISMISS
    pub decision: String,
    pub resolution_text: String,
    pub admin_token: String,
    /// White player wallet pubkey (needed to build the on-chain instruction)
    pub white_wallet: String,
    /// Black player wallet pubkey
    pub black_wallet: String,
}

#[derive(Serialize)]
pub struct ResolveDisputeResp {
    pub ok: bool,
    pub tx_sig: String,
}

// ── Handlers ──────────────────────────────────────────────────────────────────

/// POST /dispute/notify
/// Player calls this after submitting the on-chain dispute_game tx.
pub async fn notify_dispute(
    State(state): State<AppState>,
    Json(req): Json<NotifyDisputeReq>,
) -> Result<Json<NotifyDisputeResp>, StatusCode> {
    let pool = state.store.pool();
    let disputes = DisputeRepository::new(pool.clone());

    disputes
        .insert(req.game_id, &req.challenger_wallet, &req.reason)
        .await
        .map_err(|e| {
            error!("[dispute] DB insert failed: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    info!(
        "[dispute] Game {} disputed by {} — tx {}",
        req.game_id, req.challenger_wallet, req.tx_signature
    );

    send_dispute_notification_email(req.game_id, &req.challenger_wallet, &req.reason).await;

    Ok(Json(NotifyDisputeResp {
        ok: true,
        case_id: format!("DISP-{}", req.game_id),
    }))
}

/// GET /dispute/:game_id
pub async fn get_dispute_status(
    State(state): State<AppState>,
    Path(game_id): Path<i64>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let pool = state.store.pool();
    let disputes = DisputeRepository::new(pool);

    match disputes.get(game_id).await.map_err(|e| {
        error!("[dispute] DB get failed: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })? {
        Some(rec) => Ok(Json(serde_json::json!({
            "game_id": rec.game_id,
            "status": rec.status,
            "decision": rec.decision,
            "resolution_text": rec.resolution_text,
            "tx_sig": rec.tx_sig,
            "notified_at": rec.notified_at,
            "resolved_at": rec.resolved_at,
        }))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// POST /admin/dispute/resolve
/// Moderator decision → builds + signs resolve_dispute on-chain → emails both players.
pub async fn resolve_dispute(
    State(state): State<AppState>,
    Json(req): Json<ResolveDisputeReq>,
) -> Result<Json<ResolveDisputeResp>, StatusCode> {
    // Authenticate admin token
    let expected = env::var("ADMIN_TOKEN").unwrap_or_default();
    if expected.is_empty() || req.admin_token != expected {
        warn!("[dispute] resolve rejected — bad admin token");
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Load dispute_authority keypair
    let authority_key = env::var("DISPUTE_AUTHORITY_KEYPAIR").map_err(|_| {
        error!("[dispute] DISPUTE_AUTHORITY_KEYPAIR not set");
        StatusCode::SERVICE_UNAVAILABLE
    })?;
    let authority =
        solana_sdk::signature::Keypair::from_base58_string(&authority_key);

    // Determine winner pubkey
    let winner: Option<Pubkey> = match req.decision.as_str() {
        "WHITE_WINS" => Some(
            Pubkey::from_str(&req.white_wallet).map_err(|_| StatusCode::BAD_REQUEST)?,
        ),
        "BLACK_WINS" => Some(
            Pubkey::from_str(&req.black_wallet).map_err(|_| StatusCode::BAD_REQUEST)?,
        ),
        "DRAW" | "DISMISS" => None,
        _ => return Err(StatusCode::BAD_REQUEST),
    };

    // Build and send the on-chain resolve_dispute instruction
    let tx_sig = submit_resolve_dispute_tx(
        &state,
        &authority,
        req.game_id as u64,
        &req.resolution_text,
        winner,
        &req.white_wallet,
        &req.black_wallet,
    )
    .await
    .map_err(|e| {
        error!("[dispute] on-chain resolve failed: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Update DB
    let pool = state.store.pool();
    let disputes = DisputeRepository::new(pool);
    disputes
        .set_resolved(req.game_id, &req.decision, &req.resolution_text, &tx_sig)
        .await
        .map_err(|e| {
            error!("[dispute] DB update failed: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Email both players
    send_resolution_email(
        req.game_id,
        &req.decision,
        &req.resolution_text,
        &tx_sig,
        &req.white_wallet,
        &req.black_wallet,
    )
    .await;

    info!(
        "[dispute] Game {} resolved — {} — tx {}",
        req.game_id, req.decision, tx_sig
    );

    Ok(Json(ResolveDisputeResp { ok: true, tx_sig }))
}

// ── Solana helpers ────────────────────────────────────────────────────────────

async fn submit_resolve_dispute_tx(
    state: &AppState,
    authority: &solana_sdk::signature::Keypair,
    game_id: u64,
    resolution: &str,
    winner: Option<Pubkey>,
    white_wallet: &str,
    black_wallet: &str,
) -> anyhow::Result<String> {
    use solana_client::nonblocking::rpc_client::RpcClient;
    use solana_sdk::{
        instruction::{AccountMeta, Instruction},
        transaction::Transaction,
    };

    let rpc = RpcClient::new(state.config.solana_rpc_url.clone());
    let program_id = Pubkey::from_str(&state.config.program_id)?;

    let game_id_bytes = game_id.to_le_bytes();
    let (game_pda, _) = Pubkey::find_program_address(&[b"game", &game_id_bytes], &program_id);
    let (dispute_pda, _) =
        Pubkey::find_program_address(&[b"dispute", &game_id_bytes], &program_id);
    let (escrow_pda, _) =
        Pubkey::find_program_address(&[b"escrow", &game_id_bytes], &program_id);

    let white = Pubkey::from_str(white_wallet)?;
    let black = Pubkey::from_str(black_wallet)?;

    // Encode the instruction data using Anchor discriminator for resolve_dispute
    // Discriminator = sha256("global:resolve_dispute")[..8]
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(b"global:resolve_dispute");
    let disc: [u8; 8] = hasher.finalize()[..8].try_into().unwrap();

    // Encode args: resolution (String) + winner (Option<Pubkey>)
    let mut data = disc.to_vec();
    data.extend_from_slice(&(game_id as u64).to_le_bytes()); // game_id arg
    // resolution string: 4-byte LE len + bytes
    let res_bytes = resolution.as_bytes();
    data.extend_from_slice(&(res_bytes.len() as u32).to_le_bytes());
    data.extend_from_slice(res_bytes);
    // winner: Option<Pubkey> — 0 = None, 1 = Some(32 bytes)
    match winner {
        None => data.push(0),
        Some(w) => {
            data.push(1);
            data.extend_from_slice(w.as_ref());
        }
    }

    let accounts = vec![
        AccountMeta::new(game_pda, false),
        AccountMeta::new(dispute_pda, false),
        AccountMeta::new(escrow_pda, false),
        AccountMeta::new_readonly(authority.pubkey(), true),
        AccountMeta::new(white, false),
        AccountMeta::new(black, false),
        AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
    ];

    let ix = Instruction {
        program_id,
        accounts,
        data,
    };

    let blockhash = rpc.get_latest_blockhash().await?;
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&authority.pubkey()),
        &[authority],
        blockhash,
    );

    let sig = rpc.send_and_confirm_transaction(&tx).await?;
    Ok(sig.to_string())
}

// ── Email helpers ─────────────────────────────────────────────────────────────

async fn send_dispute_notification_email(game_id: i64, challenger: &str, reason: &str) {
    let key = match env::var("SENDGRID_API_KEY") {
        Ok(k) if !k.is_empty() => k,
        _ => {
            warn!("[dispute] SENDGRID_API_KEY not set — skipping email");
            return;
        }
    };

    let body = serde_json::json!({
        "personalizations": [{ "to": [{ "email": DISPUTE_NOTIFY_EMAIL }] }],
        "from": { "email": FROM_EMAIL, "name": "XFChess Fair Play" },
        "subject": format!("[DISPUTE] Game #{game_id} — dispute raised"),
        "content": [{
            "type": "text/plain",
            "value": format!(
                "A dispute has been raised.\n\nGame ID: {game_id}\nChallenger wallet: {challenger}\nReason: {reason}\n\nResolve via:\nPOST /admin/dispute/resolve\n{{ \"game_id\": {game_id}, \"decision\": \"WHITE_WINS|BLACK_WINS|DRAW|DISMISS\", \"resolution_text\": \"...\", \"admin_token\": \"<token>\", \"white_wallet\": \"...\", \"black_wallet\": \"...\" }}"
            )
        }]
    });

    send_sendgrid(&key, body).await;
}

async fn send_resolution_email(
    game_id: i64,
    decision: &str,
    resolution_text: &str,
    tx_sig: &str,
    white_wallet: &str,
    black_wallet: &str,
) {
    let key = match env::var("SENDGRID_API_KEY") {
        Ok(k) if !k.is_empty() => k,
        _ => {
            warn!("[dispute] SENDGRID_API_KEY not set — skipping resolution email");
            return;
        }
    };

    let solscan = format!("https://solscan.io/tx/{tx_sig}");
    let body_text = format!(
        "Your dispute for game #{game_id} has been resolved.\n\nDecision: {decision}\nReason: {resolution_text}\n\nTransaction: {solscan}\n\nTo appeal within 14 days, email fairplay@xfchess.com with your wallet address and game ID."
    );

    for wallet in [white_wallet, black_wallet] {
        let body = serde_json::json!({
            "personalizations": [{ "to": [{ "email": DISPUTE_NOTIFY_EMAIL }] }],
            "from": { "email": FROM_EMAIL, "name": "XFChess Fair Play" },
            "subject": format!("[RESOLVED] Dispute for game #{game_id} — {decision}"),
            "content": [{ "type": "text/plain", "value": format!("Wallet: {wallet}\n\n{body_text}") }]
        });
        send_sendgrid(&key, body).await;
    }
}

async fn send_sendgrid(api_key: &str, body: serde_json::Value) {
    let client = reqwest::Client::new();
    let url = env::var("SENDGRID_API_URL")
        .unwrap_or_else(|_| "https://api.sendgrid.com/v3/mail/send".to_string());
    if let Err(e) = client
        .post(&url)
        .header("Authorization", format!("Bearer {api_key}"))
        .json(&body)
        .send()
        .await
    {
        error!("[dispute] SendGrid request failed: {e}");
    }
}
