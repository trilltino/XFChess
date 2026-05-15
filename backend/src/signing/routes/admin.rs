use axum::{
    extract::{State, Query},
    http::StatusCode,
    response::Json,
    routing::get,
    Router,
};
use crate::signing::AppState;
use crate::db::repository::GameRepository;
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
pub struct ListQuery {
    pub limit: Option<i32>,
}

pub fn admin_routes() -> Router<AppState> {
    Router::new()
        .route("/admin/players", get(list_players))
        .route("/admin/active-sessions", get(list_active_sessions))
        .route("/admin/feepayer-balance", get(get_feepayer_balance))
        .route("/admin/wallet-balances", get(get_wallet_balances))
        .route("/admin/anti-cheat/reports", get(anti_cheat_reports))
}

async fn anti_cheat_reports(State(_state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    // Mock data until the `xfchess-anticheat` crate and verdicts table are fully implemented.
    Ok(Json(json!({
        "reports": [
            {
                "game_id": 1001,
                "white": "A1B2...3C4D",
                "black": "C9D8...7E6F",
                "suspect": "Black",
                "verdict": "Flag",
                "wager": "0.5 SOL",
                "score": 0.88,
                "reason": "T1 overlap 89%, consistent 1.5s move latency",
                "status": "Disputed"
            },
            {
                "game_id": 1045,
                "white": "F5E4...D3C2",
                "black": "B1A0...9Z8Y",
                "suspect": "White",
                "verdict": "Review",
                "wager": "1.0 SOL",
                "score": 0.65,
                "reason": "High CPL deviation from baseline",
                "status": "Disputed"
            }
        ]
    })))
}

async fn list_players(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let limit = query.limit.unwrap_or(50);
    let players = state.store.list_players(limit).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let players_json: Vec<_> = players.into_iter().map(|(wallet, username, kyc_status)| {
        json!({
            "wallet": wallet,
            "username": username,
            "kyc_status": kyc_status,
        })
    }).collect();

    Ok(Json(json!({ "players": players_json })))
}

async fn list_active_sessions(State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let repo = GameRepository::new(state.store.pool());
    let sessions = repo.list_active_sessions().await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(Json(json!({ "sessions": sessions })))
}

async fn get_feepayer_balance(State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    use solana_sdk::signer::Signer;
    let rpc = crate::signing::solana::make_rpc(&state.config.solana_rpc_url);
    // Use the next keypair from the pool for balance queries
    let feepayer_pubkey = state.feepayer.next().pubkey();
    let balance = rpc.get_balance(&feepayer_pubkey)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let sol = balance as f64 / 1_000_000_000.0;
    
    Ok(Json(json!({ 
        "balance_lamports": balance,
        "balance_sol": format!("{:.4} SOL", sol),
        "pubkey": feepayer_pubkey.to_string()
    })))
}

async fn get_wallet_balances(State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    use solana_sdk::signer::Signer;
    let rpc = crate::signing::solana::make_rpc(&state.config.solana_rpc_url);

    let get_bal = |pubkey: &solana_sdk::pubkey::Pubkey| -> Result<(u64, String), StatusCode> {
        let balance = rpc.get_balance(pubkey).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let sol = format!("{:.4} SOL", balance as f64 / 1_000_000_000.0);
        Ok((balance, sol))
    };

    // Retrieve the next fee-payer keypair from the pool for balance queries
    let fp_pk = state.feepayer.next().pubkey();
    let vps_pk = state.vps_authority.pubkey();
    let kyc_pk = state.kyc_authority.pubkey();
    let treasury_pk = state.host_treasury_pubkey;

    let (fp_bal, fp_sol) = get_bal(&fp_pk)?;
    let (vps_bal, vps_sol) = get_bal(&vps_pk)?;
    let (kyc_bal, kyc_sol) = get_bal(&kyc_pk)?;
    let (treasury_bal, treasury_sol) = get_bal(&treasury_pk)?;

    Ok(Json(json!({
        "feepayer": { "pubkey": fp_pk.to_string(), "balance_lamports": fp_bal, "balance_sol": fp_sol },
        "vps_signer": { "pubkey": vps_pk.to_string(), "balance_lamports": vps_bal, "balance_sol": vps_sol },
        "kyc_signer": { "pubkey": kyc_pk.to_string(), "balance_lamports": kyc_bal, "balance_sol": kyc_sol },
        "treasury": { "pubkey": treasury_pk.to_string(), "balance_lamports": treasury_bal, "balance_sol": treasury_sol },
    })))
}
