use super::*;

#[derive(Debug)]
pub enum CancelOutcome {
    Refunded(solana_sdk::signature::Signature),
    NothingToRefund(String),
}

pub(super) fn is_user_rejection(e: &str) -> bool {
    let l = e.to_lowercase();
    l.contains("reject")
        || l.contains("denied")
        || l.contains("declined")
        || l.contains("user cancel")
        || l.contains("user closed")
}

pub fn cancel_game_on_chain(
    rpc_url: String,
    program_id: Pubkey,
    wallet_pubkey: Pubkey,
    game_id: u64,
) -> Result<CancelOutcome, String> {
    use crate::multiplayer::solana::tauri_signer::sign_and_send_via_tauri;
    use crate::solana::instructions::cancel_game_ix;

    let rpc = RpcClient::new_with_commitment(rpc_url.clone(), CommitmentConfig::confirmed());
    let game_pda =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let data = match rpc.get_account_data(&game_pda) {
        Ok(d) => d,
        Err(e) => {
            let s = e.to_string();
            if s.contains("not found") || s.contains("AccountNotFound") {
                return Err(
                    "Game account missing; checking transaction history and settlement is required"
                        .to_string(),
                );
            }
            return Err(format!("fetch game account: {s}"));
        }
    };
    use crate::multiplayer::solana::wager_recovery;
    use xfchess_game::state::GameStatus;

    let Some(game) = wager_recovery::decode_game(&data) else {
        return Err("Game account unreadable; refund status is unknown".to_string());
    };
    let (white, black) = (game.white, game.black);
    if game.is_delegated {
        return Err(
            "Game is still delegated to the Ephemeral Rollup; it must be undelegated before a refund"
                .to_string(),
        );
    }

    match game.status {
        GameStatus::WaitingForOpponent | GameStatus::Active => {}
        GameStatus::Cancelled => {
            // A prior cancellation may have landed without acknowledgement.
            // The idempotent Cancelled branch refunds only the escrow balance still held.
            let escrow = wager_recovery::escrow_pda(&program_id, game_id);
            let held = rpc
                .get_balance(&escrow)
                .map_err(|e| format!("Game is cancelled; escrow balance unknown: {e}"))?;
            if held < game.wager_amount {
                return Ok(CancelOutcome::NothingToRefund(
                    "Game was already cancelled and its escrow has been refunded".to_string(),
                ));
            }
        }
        GameStatus::Settled => {
            return Ok(CancelOutcome::NothingToRefund(
                "game already settled".to_string(),
            ));
        }
        GameStatus::Finished => {
            return Err(
                "Game finished; its payout is settled by the settlement worker, not a cancellation"
                    .to_string(),
            );
        }
        GameStatus::Expired => {
            return Err("Game expired; refund status requires chain reconciliation".to_string());
        }
        status => {
            // Pending / Inactive / Disputed — the on-chain cancel would reject
            // these states; don't burn a wallet popup on a doomed transaction.
            return Err(format!(
                "game {game_id} cannot be cancelled right now (status {status:?})"
            ));
        }
    }

    // After 24 hours, reclaim an unjoined wager with withdraw_expired_wager.
    // Older programs may reject earlier cancel_game calls with error 3011.
    let unjoined = game.status == GameStatus::WaitingForOpponent;
    let withdrawable_at = game.created_at + 86_400;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let ix = if unjoined && now > withdrawable_at {
        crate::solana::instructions::withdraw_expired_wager_ix(program_id, wallet_pubkey, game_id)
    } else {
        cancel_game_ix(program_id, wallet_pubkey, white, black, game_id)
            .map_err(|e| format!("build cancel_game_ix: {e}"))?
    };
    let explain = |e: String| -> String {
        if unjoined
            && (e.contains("0xbc3")
                || e.contains("Custom(3011)")
                || e.contains("AccountNotSystemOwned"))
        {
            let hours = ((withdrawable_at - now).max(0) + 3599) / 3600;
            format!(
                "This network's game program can't cancel a game before an opponent joins yet. \
                 Your stake is safe in escrow and can be reclaimed here in about {hours}h. ({e})"
            )
        } else {
            e
        }
    };

    let mut attempt = 0;
    loop {
        attempt += 1;
        match sign_and_send_via_tauri(
            &rpc_url,
            wallet_pubkey,
            std::slice::from_ref(&ix),
            &[],
            "Cancelling wagered game",
        ) {
            Ok(sig) => {
                // Fast submission can return an unconfirmed signature after two seconds.
                // Keep the wager ledger until cancellation is confirmed.
                let mut confirmed = false;
                for _ in 0..20 {
                    match rpc
                        .get_signature_status_with_commitment(&sig, CommitmentConfig::confirmed())
                    {
                        Ok(Some(Ok(()))) => {
                            confirmed = true;
                            break;
                        }
                        Ok(Some(Err(e))) => {
                            return Err(explain(format!(
                                "Cancellation transaction {sig} failed: {e:?}"
                            )));
                        }
                        Ok(None) | Err(_) => {
                            std::thread::sleep(std::time::Duration::from_millis(250));
                        }
                    }
                }
                if !confirmed {
                    return Err(format!(
                        "Cancellation outcome unknown for signature {sig}; check the chain before retrying"
                    ));
                }
                info!(
                    "[CANCEL_GAME] game {} cancelled on-chain, sig {}",
                    game_id, sig
                );
                return Ok(CancelOutcome::Refunded(sig));
            }
            Err(e) => {
                // A program error is deterministic: retrying would only
                // re-prompt the wallet for the same rejection.
                if attempt >= 3 || is_user_rejection(&e) || e.contains("custom program error") {
                    return Err(explain(e));
                }
                warn!(
                    "[CANCEL_GAME] attempt {} failed for game {}: {}, retrying...",
                    attempt, game_id, e
                );
                std::thread::sleep(std::time::Duration::from_millis(1000));
            }
        }
    }
}

pub fn spawn_cancel_lobby(
    rpc_url: String,
    program_id: Pubkey,
    wallet_pubkey: Option<Pubkey>,
    game_id: u64,
    wager_lamports: u64,
    node_id: Option<String>,
    tx: oneshot::Sender<Result<CancelOutcome, String>>,
) {
    crate::core::net_task::spawn_detached("lobby", move || {
        let on_chain = if wager_lamports > 0 {
            let wallet = wallet_pubkey
                .ok_or_else(|| "Wallet unavailable; the wager could not be cancelled".to_string());
            wallet.and_then(|wallet| cancel_game_on_chain(rpc_url, program_id, wallet, game_id))
        } else {
            Ok(CancelOutcome::NothingToRefund(
                "free game; no escrow to refund".to_string(),
            ))
        };

        let relay_error = node_id.and_then(|node_id| {
            crate::multiplayer::vps_client::p2p_leave_game_fast(game_id.to_string(), &node_id).err()
        });
        if let Some(error) = relay_error {
            warn!(
                "[LOBBY] Relay cleanup failed for cancelled game {}: {}",
                game_id, error
            );
        }

        let result = on_chain;
        let _ = tx.send(result);
    });
}
