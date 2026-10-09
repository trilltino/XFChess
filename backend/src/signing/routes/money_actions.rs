use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use solana_sdk::{pubkey::Pubkey, signature::Signature, transaction::VersionedTransaction};
use std::str::FromStr;

use crate::signing::{
    storage::money_action::{MoneyActionRecord, NewMoneyAction},
    AppState,
};

#[derive(Debug, Deserialize)]
pub struct CreateMoneyActionReq {
    pub action_type: String,
    pub scope_type: String,
    pub game_id: Option<i64>,
    pub tournament_id: Option<i64>,
    pub wallet: Option<String>,
    pub signature: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MoneyActionScopeQuery {
    pub game_id: Option<i64>,
    pub tournament_id: Option<i64>,
    pub wallet: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MoneyActionList {
    pub money_actions: Vec<MoneyActionRecord>,
}

pub fn money_action_routes() -> Router<AppState> {
    Router::new()
        .route("/money-actions", post(create_money_action))
        .route("/money-actions/{id}", get(get_money_action))
        .route("/money-actions/by-scope", get(money_actions_by_scope))
}

pub fn admin_money_action_routes() -> Router<AppState> {
    Router::new()
        .route("/admin/money-actions/by-scope", get(money_actions_by_scope))
        .route(
            "/admin/money-actions/{id}/retry-reconcile",
            post(admin_retry_reconcile),
        )
        .route(
            "/admin/money-actions/{id}/mark-review",
            post(admin_mark_review),
        )
}

fn clean_opt(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn valid_kind(value: &str) -> bool {
    value
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && (1..=64).contains(&value.len())
}

async fn create_money_action(
    State(state): State<AppState>,
    Json(req): Json<CreateMoneyActionReq>,
) -> Result<Json<MoneyActionRecord>, StatusCode> {
    if !valid_kind(&req.action_type) || !valid_kind(&req.scope_type) {
        return Err(StatusCode::BAD_REQUEST);
    }
    if let Some(sig) = req.signature.as_deref() {
        Signature::from_str(sig).map_err(|_| StatusCode::BAD_REQUEST)?;
    }
    if let Some(wallet) = req.wallet.as_deref() {
        Pubkey::from_str(wallet).map_err(|_| StatusCode::BAD_REQUEST)?;
    }

    let status = if req.signature.as_ref().is_some_and(|s| !s.trim().is_empty()) {
        "submitted"
    } else {
        "signing"
    };

    let record = state
        .money_actions
        .create_or_get(NewMoneyAction {
            action_type: req.action_type,
            scope_type: req.scope_type,
            game_id: req.game_id,
            tournament_id: req.tournament_id,
            wallet: clean_opt(req.wallet),
            signature: clean_opt(req.signature),
            status: status.to_string(),
            reason: clean_opt(req.reason),
        })
        .await
        .map_err(|e| {
            tracing::error!("[money-actions] create failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    tracing::info!(
        target: "money_actions",
        event = "money_action_created",
        id = %record.id,
        action_type = %record.action_type,
        scope_type = %record.scope_type,
        game_id = ?record.game_id,
        tournament_id = ?record.tournament_id,
        wallet = ?record.wallet,
        signature = ?record.signature,
        status = %record.status
    );

    if record.signature.is_some() {
        let state_clone = state.clone();
        let id = record.id.clone();
        tokio::spawn(async move {
            reconcile_money_action_once(state_clone, id).await;
        });
    }

    Ok(Json(record))
}

async fn get_money_action(
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<MoneyActionRecord>, StatusCode> {
    let record = state
        .money_actions
        .get(&id)
        .await
        .map_err(|e| {
            tracing::error!("[money-actions] get failed id={}: {}", id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(record))
}

pub async fn money_actions_by_scope(
    State(state): State<AppState>,
    Query(q): Query<MoneyActionScopeQuery>,
) -> Result<Json<MoneyActionList>, StatusCode> {
    let records = state
        .money_actions
        .by_scope(q.game_id, q.tournament_id, q.wallet.as_deref())
        .await
        .map_err(|e| {
            tracing::error!("[money-actions] scope lookup failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(MoneyActionList {
        money_actions: records,
    }))
}

async fn admin_retry_reconcile(
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<MoneyActionRecord>, StatusCode> {
    reconcile_money_action_once(state.clone(), id.clone()).await;
    get_money_action(Path(id), State(state)).await
}

async fn admin_mark_review(
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<MoneyActionRecord>, StatusCode> {
    state
        .money_actions
        .transition(
            &id,
            "needs_admin_review",
            Some("Marked for review by admin"),
            None,
            None,
            false,
        )
        .await
        .map_err(|e| {
            tracing::error!("[money-actions] mark-review failed id={}: {}", id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)
        .map(Json)
}

fn instruction_discriminator(name: &str) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(format!("global:{name}").as_bytes());
    hasher.finalize()[..8].to_vec()
}

fn action_instruction_name(action_type: &str) -> Option<&'static str> {
    match action_type {
        "cancel_game" => Some("cancel_game"),
        "leave_tournament" => Some("leave_tournament"),
        "claim_timeout" => Some("claim_timeout"),
        "finalize_game" => Some("finalize_game"),
        "cancel_tournament" => Some("cancel_tournament"),
        "claim_prize" => Some("claim_tournament_prize"),
        "distribute_prize" | "prize_distribution" => Some("distribute_tournament_prizes"),
        _ => None,
    }
}

fn tx_has_expected_instruction(
    tx: &VersionedTransaction,
    keys: &[Pubkey],
    program_id: Pubkey,
    action_type: &str,
    game_id: Option<i64>,
    tournament_id: Option<i64>,
    wallet: Option<&str>,
) -> Result<bool, String> {
    let instruction_name = action_instruction_name(action_type)
        .ok_or_else(|| format!("unsupported money action type: {action_type}"))?;
    let discriminator = instruction_discriminator(instruction_name);
    // Positions follow the program's Accounts structs, including optional slots.
    let (is_game, min_accounts, escrow_slot, role_slots, role_signs):
        (bool, usize, Option<usize>, &[usize], bool) = match action_type {
        "cancel_game" => (true, 6, Some(1), &[2], true),
        "claim_timeout" => (true, 2, None, &[1], true),
        "finalize_game" => (true, 9, Some(5), &[3, 4, 7], false),
        "leave_tournament" => (false, 8, Some(6), &[5], true),
        "cancel_tournament" => (false, 14, Some(9), &[11], true),
        "claim_prize" => (false, 10, Some(5), &[7], true),
        "distribute_prize" | "prize_distribution" => (false, 3, Some(1), &[2], true),
        _ => unreachable!(),
    };
    let id = if is_game { game_id } else { tournament_id }
        .and_then(|id| u64::try_from(id).ok())
        .ok_or_else(|| "missing or negative action scope ID".to_string())?;
    if (is_game && tournament_id.is_some()) || (!is_game && game_id.is_some()) {
        return Err("conflicting action scope IDs".to_string());
    }
    let wallet = wallet.map(Pubkey::from_str).transpose().map_err(|e| e.to_string())?;
    let seed: &[u8] = if is_game { b"game" } else { b"tournament" };
    let escrow_seed: &[u8] = if is_game { b"escrow" } else { b"t_escrow" };
    let pda = |seed: &[u8]| Pubkey::find_program_address(&[seed, &id.to_le_bytes()], &program_id).0;
    let signer_count = tx.message.header().num_required_signatures as usize;
    Ok(tx.message.instructions().iter().any(|ix| {
        let program = keys.get(ix.program_id_index as usize).copied();
        if program != Some(program_id)
            || ix.data.len() != 16
            || !ix.data.starts_with(&discriminator)
            || ix.data.get(8..16) != Some(id.to_le_bytes().as_slice())
            || ix.accounts.len() < min_accounts
            || ix.accounts.iter().any(|index| usize::from(*index) >= keys.len())
        {
            return false;
        }
        let account = |slot: usize| ix.accounts.get(slot).and_then(|index| keys.get(*index as usize)).copied();
        if account(0) != Some(pda(seed))
            || escrow_slot.is_some_and(|slot| account(slot) != Some(pda(escrow_seed)))
        {
            return false;
        }
        if action_type == "claim_prize"
            && (account(1) != Some(pda(b"t_usdc_prize")) || account(6) != account(7))
        {
            return false;
        }
        role_slots.iter().any(|slot| {
            let index = ix.accounts[*slot] as usize;
            (!role_signs || index < signer_count)
                && wallet.is_none_or(|wallet| account(*slot) == Some(wallet))
        })
    }))
}

fn resolved_account_keys(
    tx: &VersionedTransaction,
    writable: &[String],
    readonly: &[String],
) -> Result<Vec<Pubkey>, String> {
    let (mut expected_writable, mut expected_readonly) = (0, 0);
    for lookup in tx.message.address_table_lookups().unwrap_or_default() {
        expected_writable += lookup.writable_indexes.len();
        expected_readonly += lookup.readonly_indexes.len();
    }
    if writable.len() != expected_writable || readonly.len() != expected_readonly {
        return Err("missing or inconsistent resolved lookup-table addresses".to_string());
    }
    let mut keys = tx.message.static_account_keys().to_vec();
    for address in writable.iter().chain(readonly) {
        keys.push(Pubkey::from_str(address).map_err(|e| format!("invalid loaded address: {e}"))?);
    }
    Ok(keys)
}

fn landed_transaction(
    rpc: &solana_client::rpc_client::RpcClient,
    signature: &Signature,
) -> Result<Option<(VersionedTransaction, Vec<Pubkey>)>, String> {
    let statuses = rpc
        .get_signature_statuses_with_history(&[*signature])
        .map_err(|e| format!("signature status lookup failed: {e}"))?;
    let status = statuses.value.first().and_then(|entry| entry.as_ref());
    let Some(status) = status else {
        return Ok(None);
    };
    if !status.satisfies_commitment(solana_commitment_config::CommitmentConfig::confirmed()) {
        return Ok(None);
    }
    if let Some(err) = &status.err {
        return Err(format!("transaction failed on-chain: {err:?}"));
    }
    let fetched = crate::signing::solana::fetch_transaction_v1_aware(rpc, signature)
        .map_err(|e| e.to_string())?;
    let meta = fetched.confirmed.transaction.meta.as_ref()
        .ok_or_else(|| "transaction metadata is missing".to_string())?;
    if meta.err.is_some() || meta.status.is_err() {
        return Err(format!("transaction failed on-chain: {:?}", meta.err));
    }
    if fetched.decoded.signatures.first() != Some(signature) {
        return Err("fetched transaction signature mismatch".to_string());
    }
    let keys = resolved_account_keys(
        &fetched.decoded,
        &fetched.loaded_writable_accounts,
        &fetched.loaded_readonly_accounts,
    )?;
    Ok(Some((fetched.decoded, keys)))
}

fn resolved_status_for(action_type: &str) -> (&'static str, &'static str) {
    match action_type {
        "claim_timeout" => (
            "confirmed",
            "claim_timeout landed; settlement/finalize remains separately reconcilable",
        ),
        _ => (
            "resolved",
            "expected money instruction landed successfully on-chain",
        ),
    }
}

pub async fn reconcile_money_action_once(state: AppState, id: String) {
    let Some(record) = (match state.money_actions.get(&id).await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("[money-actions] reconcile load failed id={}: {}", id, e);
            return;
        }
    }) else {
        return;
    };

    let Some(signature) = record.signature.clone() else {
        let _ = state
            .money_actions
            .transition(
                &id,
                "signing",
                Some("Waiting for wallet signature"),
                None,
                None,
                false,
            )
            .await;
        return;
    };

    let sig = match Signature::from_str(&signature) {
        Ok(sig) => sig,
        Err(e) => {
            let _ = state
                .money_actions
                .transition(
                    &id,
                    "failed",
                    Some("Malformed transaction signature"),
                    Some(&e.to_string()),
                    None,
                    true,
                )
                .await;
            return;
        }
    };

    let rpc = state.solana_rpc.clone();
    let landed = tokio::task::spawn_blocking(move || landed_transaction(&rpc, &sig))
        .await
        .unwrap_or_else(|e| Err(format!("transaction lookup worker failed: {e}")));
    match landed {
        Ok(None) => {
            let next = Utc::now().timestamp() + 30;
            let _ = state
                .money_actions
                .transition(
                    &id,
                    "pending_reconciliation",
                    Some("Transaction is not visible on-chain yet"),
                    None,
                    Some(next),
                    true,
                )
                .await;
            tracing::info!(
                target: "money_actions",
                event = "reconcile_pending",
                id = %id,
                signature = %signature,
                action_type = %record.action_type
            );
        }
        Ok(Some((tx, keys))) => {
            match tx_has_expected_instruction(
                &tx,
                &keys,
                state.program_id,
                &record.action_type,
                record.game_id,
                record.tournament_id,
                record.wallet.as_deref(),
            ) {
                Ok(true) => {
                    let (status, reason) = resolved_status_for(&record.action_type);
                    let _ = state
                        .money_actions
                        .transition(&id, status, Some(reason), None, None, true)
                        .await;
                    tracing::info!(
                        target: "money_actions",
                        event = "reconcile_resolved",
                        id = %id,
                        signature = %signature,
                        action_type = %record.action_type,
                        status = %status
                    );
                }
                Ok(false) => {
                    let _ = state
                    .money_actions
                    .transition(
                        &id,
                        "needs_admin_review",
                        Some("Landed transaction does not contain the expected instruction/signers"),
                        None,
                        None,
                        true,
                    )
                    .await;
                }
                Err(e) => {
                    let _ = state
                        .money_actions
                        .transition(
                            &id,
                            "needs_admin_review",
                            Some("Could not validate expected instruction"),
                            Some(&e),
                            None,
                            true,
                        )
                        .await;
                }
            }
        }
        Err(e) => {
            let status = if e.contains("failed on-chain") {
                "failed"
            } else if record.attempt_count >= 5 {
                "needs_admin_review"
            } else {
                "pending_reconciliation"
            };
            let next_retry_at = if status == "pending_reconciliation" {
                Some(Utc::now().timestamp() + 60)
            } else {
                None
            };
            let _ = state
                .money_actions
                .transition(
                    &id,
                    status,
                    Some("Reconciliation could not prove the money action yet"),
                    Some(&e),
                    next_retry_at,
                    true,
                )
                .await;
            tracing::warn!(
                target: "money_actions",
                event = "reconcile_error",
                id = %id,
                signature = %signature,
                action_type = %record.action_type,
                status = %status,
                error = %e
            );
        }
    }
}
