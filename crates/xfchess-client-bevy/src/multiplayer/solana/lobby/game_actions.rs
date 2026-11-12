use super::*;

use crate::solana::instructions::{claim_draw_ix, resign_ix};

/// What the player does to a running wagered game that the Game account must record
/// for it to settle: a resignation or an agreed draw only pays out once the account
/// says `Finished`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameChainAction {
    Resign,
    OfferDraw,
    AcceptDraw,
    /// A formal FIDE claim (Articles 9.2/9.3). Not withdrawable once submitted.
    ClaimDraw {
        repetition: bool,
        /// The irrevocably declared legal intended move, if the claim is prospective.
        intended: Option<[u8; 5]>,
        /// The position the player saw: a claim never silently moves to a later one.
        expected_nonce: u64,
    },
}

impl GameChainAction {
    fn wallet_label(self) -> &'static str {
        match self {
            Self::Resign => "Resigning",
            Self::OfferDraw => "Offering draw",
            Self::AcceptDraw => "Accepting draw",
            Self::ClaimDraw { .. } => "Claiming draw",
        }
    }

    fn instruction(
        self,
        program_id: Pubkey,
        game_id: u64,
        player: Pubkey,
    ) -> Result<solana_sdk::instruction::Instruction, String> {
        match self {
            Self::Resign => resign_ix(program_id, game_id, player),
            Self::OfferDraw => offer_draw_ix(program_id, game_id, player),
            Self::AcceptDraw => accept_draw_ix(program_id, game_id, player),
            // Needs live nonces: built in `submit_game_action`.
            Self::ClaimDraw { .. } => Err(anyhow::anyhow!("claim needs live nonces")),
        }
        .map_err(|e| format!("build {self:?} instruction: {e}"))
    }
}

/// A delegated game lives on the rollup, not the base layer: a transaction sent to
/// the base RPC for it is refused. Pick the endpoint the account is on right now.
fn resolve_game_rpc(base_rpc_url: &str, game_id: u64) -> Result<String, String> {
    let program_id: Pubkey = SOLANA_PROGRAM_ID
        .parse()
        .map_err(|e| format!("program id: {e}"))?;
    let delegation_program: Pubkey = crate::multiplayer::rollup::magicblock::DELEGATION_PROGRAM_ID
        .parse()
        .map_err(|e| format!("delegation program id: {e}"))?;
    let pda = Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let base =
        RpcClient::new_with_commitment(base_rpc_url.to_string(), CommitmentConfig::confirmed());
    match base.get_account(&pda) {
        Ok(account) if account.owner == delegation_program => {
            crate::multiplayer::network::vps::game::game_er_endpoint(game_id)
        }
        Ok(_) => Ok(base_rpc_url.to_string()),
        Err(e) => Err(format!(
            "Transaction simulation: could not read the game: {e}"
        )),
    }
}

/// Failures that happen before the wallet is asked to sign (the simulation, the
/// blockhash) say nothing about the player's intent and are worth retrying: a draw
/// acceptance can race the offer reaching the chain, and a game can move between
/// the base layer and the rollup mid-settlement.
fn is_retryable(error: &str) -> bool {
    error.starts_with("Transaction simulation") || error.starts_with("get_latest_blockhash")
}

const MAX_ATTEMPTS: u32 = 6;
const RETRY_DELAY: std::time::Duration = std::time::Duration::from_secs(2);

/// Send `action` for `wallet`, signed in the wallet, to wherever the game lives.
/// Blocking; call from a background thread.
pub fn submit_game_action(
    action: GameChainAction,
    base_rpc_url: &str,
    wallet: Pubkey,
    game_id: u64,
) -> Result<solana_sdk::signature::Signature, String> {
    use crate::multiplayer::solana::tauri_signer::sign_and_send_via_tauri;

    let program_id: Pubkey = SOLANA_PROGRAM_ID
        .parse()
        .map_err(|e| format!("program id: {e}"))?;
    let fixed_ix = match action {
        GameChainAction::ClaimDraw { .. } => None,
        _ => Some(action.instruction(program_id, game_id, wallet)?),
    };
    let mut last_error = String::new();
    for attempt in 1..=MAX_ATTEMPTS {
        let result = resolve_game_rpc(base_rpc_url, game_id).and_then(|rpc_url| {
            let ix = match (&fixed_ix, action) {
                (Some(ix), _) => ix.clone(),
                (
                    None,
                    GameChainAction::ClaimDraw {
                        repetition,
                        intended,
                        expected_nonce,
                    },
                ) => claim_instruction(
                    &rpc_url,
                    program_id,
                    game_id,
                    wallet,
                    repetition,
                    intended,
                    expected_nonce,
                )?,
                (None, _) => return Err("instruction unavailable".to_string()),
            };
            sign_and_send_via_tauri(
                &rpc_url,
                wallet,
                std::slice::from_ref(&ix),
                &[],
                action.wallet_label(),
            )
        });
        match result {
            Ok(sig) => {
                info!("[GAME-ACTION] {action:?} confirmed on-chain for game {game_id}: {sig}");
                return Ok(sig);
            }
            Err(e) if attempt < MAX_ATTEMPTS && is_retryable(&e) => {
                warn!("[GAME-ACTION] {action:?} attempt {attempt} not ready ({e}); retrying");
                last_error = e;
                std::thread::sleep(RETRY_DELAY);
            }
            Err(e) => {
                error!("[GAME-ACTION] {action:?} failed for game {game_id}: {e}");
                return Err(e);
            }
        }
    }
    Err(last_error)
}

/// The next unused claim nonce kept in the versioned FIDE tail of a Game account,
/// or `None` for a legacy-sized account that has no FIDE state.
pub fn claim_nonce_from_account(data: &[u8]) -> Option<u64> {
    use xfchess_game::fide::{FIDE_GAME_SPACE, LEGACY_GAME_SPACE};
    if data.len() != FIDE_GAME_SPACE {
        return None;
    }
    let tail = data.get(LEGACY_GAME_SPACE..)?;
    Some(u64::from_le_bytes(tail.get(17..25)?.try_into().ok()?))
}

/// Build a claim against the live position: the nonce it adjudicates against
/// and the next unused claim nonce come from the account itself, never from
/// client-local state.
fn claim_instruction(
    rpc_url: &str,
    program_id: Pubkey,
    game_id: u64,
    wallet: Pubkey,
    repetition: bool,
    intended: Option<[u8; 5]>,
    expected_nonce: u64,
) -> Result<solana_sdk::instruction::Instruction, String> {
    let pda = Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let client = RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());
    let account = client
        .get_account(&pda)
        .map_err(|e| format!("Transaction simulation: could not read the game: {e}"))?;
    let game = crate::multiplayer::solana::wager_recovery::decode_game(&account.data)
        .ok_or_else(|| "the game account could not be decoded".to_string())?;
    if game.nonce != expected_nonce {
        return Err("The position changed before the claim was sent; nothing was claimed.".to_string());
    }
    let claim_nonce = claim_nonce_from_account(&account.data)
        .ok_or_else(|| "this game predates FIDE claims and cannot take one".to_string())?;
    claim_draw_ix(
        program_id,
        game_id,
        wallet,
        None,
        game.nonce,
        claim_nonce,
        intended,
        repetition,
    )
    .map_err(|e| format!("build claim instruction: {e}"))
}

/// What the program decided about a filed claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaimResult {
    /// Correct: the Game account is finished as a draw.
    Drawn,
    /// Incorrect: the game goes on, and the declared move (if any) is now binding.
    Continues { mandatory_move: Option<String> },
    /// Finished some other way (the opponent's move or a timeout got there first).
    Elsewhere,
}

/// Read the result of a claim from the Game account after it confirmed. The
/// program, not this client, decides; the FIDE tail holds the binding move.
pub fn read_claim_result(base_rpc_url: &str, game_id: u64) -> Result<ClaimResult, String> {
    use xfchess_game::fide::{FIDE_GAME_SPACE, LEGACY_GAME_SPACE};
    use xfchess_game::state::{GameResult, GameStatus};
    let program_id: Pubkey = SOLANA_PROGRAM_ID
        .parse()
        .map_err(|e| format!("program id: {e}"))?;
    let rpc_url = resolve_game_rpc(base_rpc_url, game_id)?;
    let pda = Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id).0;
    let client = RpcClient::new_with_commitment(rpc_url, CommitmentConfig::confirmed());
    let account = client
        .get_account(&pda)
        .map_err(|e| format!("could not read the game: {e}"))?;
    let game = crate::multiplayer::solana::wager_recovery::decode_game(&account.data)
        .ok_or_else(|| "the game account could not be decoded".to_string())?;
    match game.status {
        GameStatus::Finished if game.result == GameResult::Draw => Ok(ClaimResult::Drawn),
        GameStatus::Active => {
            let tail = (account.data.len() == FIDE_GAME_SPACE)
                .then(|| account.data.get(LEGACY_GAME_SPACE..))
                .flatten()
                .ok_or_else(|| "this game has no FIDE state".to_string())?;
            let mandatory_move = (tail.get(11) == Some(&1))
                .then(|| tail.get(12..17))
                .flatten()
                .map(|bytes| {
                    bytes
                        .iter()
                        .take_while(|&&b| b != 0)
                        .map(|&b| b as char)
                        .collect::<String>()
                });
            Ok(ClaimResult::Continues { mandatory_move })
        }
        _ => Ok(ClaimResult::Elsewhere),
    }
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
    let rpc_url = resolve_game_rpc(&rpc_url, game_id)?;
    let sig = sign_and_send_via_tauri(&rpc_url, wallet_pubkey, &[ix], &[], "Claiming timeout")?;
    info!("[TIMEOUT] claim_timeout confirmed on-chain: {sig}");
    Ok(sig)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_failures_before_the_wallet_prompt_are_retried() {
        assert!(is_retryable(
            "Transaction simulation failed: custom program error: 0x1778"
        ));
        assert!(is_retryable("get_latest_blockhash: timed out"));
        assert!(!is_retryable("Wallet request was rejected"));
        assert!(!is_retryable("Wallet changed the requested transaction"));
        assert!(!is_retryable("Could not locate the live game (503): busy"));
    }
}
