use super::*;

pub fn spawn_offer_draw(rpc_url: String, wallet_pubkey: Pubkey, game_id: u64) {
    let program_id: Pubkey = SOLANA_PROGRAM_ID.parse().unwrap_or_default();
    crate::core::net_task::spawn_detached("lobby", move || {
        futures_lite::future::block_on(async move {
            if let Err(e) = async_offer_draw(rpc_url, wallet_pubkey, program_id, game_id).await {
                error!("[DRAW] offer_draw on-chain tx failed: {e}");
            }
        })
    });
}

pub(super) async fn async_offer_draw(
    rpc_url: String,
    wallet_pubkey: Pubkey,
    program_id: Pubkey,
    game_id: u64,
) -> Result<(), String> {
    use crate::multiplayer::solana::tauri_signer::sign_and_send_via_tauri;

    let ix = offer_draw_ix(program_id, game_id, wallet_pubkey)
        .map_err(|e| format!("build offer_draw_ix: {e}"))?;
    let sig = sign_and_send_via_tauri(&rpc_url, wallet_pubkey, &[ix], &[], "Offering draw")?;
    info!("[DRAW] offer_draw confirmed on-chain: {sig}");
    Ok(())
}

pub fn spawn_accept_draw(rpc_url: String, wallet_pubkey: Pubkey, game_id: u64) {
    let program_id: Pubkey = SOLANA_PROGRAM_ID.parse().unwrap_or_default();
    crate::core::net_task::spawn_detached("lobby", move || {
        futures_lite::future::block_on(async move {
            if let Err(e) = async_accept_draw(rpc_url, wallet_pubkey, program_id, game_id).await {
                error!("[DRAW] accept_draw on-chain tx failed: {e}");
            }
        })
    });
}

pub(super) async fn async_accept_draw(
    rpc_url: String,
    wallet_pubkey: Pubkey,
    program_id: Pubkey,
    game_id: u64,
) -> Result<(), String> {
    use crate::multiplayer::solana::tauri_signer::sign_and_send_via_tauri;

    let ix = accept_draw_ix(program_id, game_id, wallet_pubkey)
        .map_err(|e| format!("build accept_draw_ix: {e}"))?;
    let sig = sign_and_send_via_tauri(&rpc_url, wallet_pubkey, &[ix], &[], "Accepting draw")?;
    info!("[DRAW] accept_draw confirmed on-chain: {sig}");
    Ok(())
}

pub fn spawn_claim_timeout(
    rpc_url: String,
    wallet_pubkey: Pubkey,
    game_id: u64,
) -> oneshot::Receiver<Result<solana_sdk::signature::Signature, String>> {
    let program_id: Pubkey = SOLANA_PROGRAM_ID.parse().unwrap_or_default();
    let (tx, rx) = oneshot::channel();
    crate::core::net_task::spawn_detached("lobby", move || {
        futures_lite::future::block_on(async move {
            let result = async_claim_timeout(rpc_url, wallet_pubkey, program_id, game_id).await;
            if let Err(e) = &result {
                error!("[TIMEOUT] claim_timeout on-chain tx failed: {e}");
            }
            let _ = tx.send(result);
        })
    });
    rx
}

pub(super) async fn async_claim_timeout(
    rpc_url: String,
    wallet_pubkey: Pubkey,
    program_id: Pubkey,
    game_id: u64,
) -> Result<solana_sdk::signature::Signature, String> {
    use crate::multiplayer::solana::tauri_signer::sign_and_send_via_tauri;

    let ix = claim_timeout_ix(program_id, game_id, wallet_pubkey)
        .map_err(|e| format!("build claim_timeout_ix: {e}"))?;
    let sig = sign_and_send_via_tauri(&rpc_url, wallet_pubkey, &[ix], &[], "Claiming timeout")?;
    info!("[TIMEOUT] claim_timeout confirmed on-chain: {sig}");
    Ok(sig)
}
