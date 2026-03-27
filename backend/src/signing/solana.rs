use anyhow::{anyhow, Result};
use tracing::warn;
use sha2::{Digest, Sha256};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    instruction::{AccountMeta, Instruction},
    message::Message,
    pubkey::Pubkey,
    signature::{Keypair, Signature},
    signer::Signer,
    system_instruction,
    transaction::{Transaction, VersionedTransaction},
};

pub const GAME_SEED: &[u8] = b"game";
pub const MOVE_LOG_SEED: &[u8] = b"move_log";
pub const SESSION_DELEGATION_SEED: &[u8] = b"session_delegation";
pub const PROFILE_SEED: &[u8] = b"profile";
pub const WAGER_ESCROW_SEED: &[u8] = b"escrow";

/// MagicBlock magic context account (ER-only).
const MAGIC_CONTEXT_PUBKEY: &str = "MagicContext1111111111111111111111111111111";
/// MagicBlock magic program (ER-only).
const MAGIC_PROGRAM_PUBKEY: &str = "Magic11111111111111111111111111111111111111";

fn anchor_discriminator(name: &str) -> [u8; 8] {
    let mut hasher = Sha256::new();
    hasher.update(format!("global:{name}"));
    hasher.finalize()[..8].try_into().unwrap()
}

fn borsh_string(s: &str) -> Vec<u8> {
    let mut v = (s.len() as u32).to_le_bytes().to_vec();
    v.extend_from_slice(s.as_bytes());
    v
}

pub fn record_move_ix(
    program_id: &Pubkey,
    session_pubkey: &Pubkey,
    game_id: u64,
    move_str: &str,
    next_fen: &str,
) -> Instruction {
    let game_pda = Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], program_id).0;
    let move_log_pda =
        Pubkey::find_program_address(&[MOVE_LOG_SEED, &game_id.to_le_bytes()], program_id).0;

    let mut data = anchor_discriminator("record_move").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());
    data.extend(borsh_string(move_str));
    data.extend(borsh_string(next_fen));

    Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new(move_log_pda, false),
            AccountMeta::new_readonly(*session_pubkey, true),
        ],
        data,
    }
}

/// Build an `undelegate_game` instruction (sent to the ER endpoint).
/// Commits the ER state for game + move_log back to devnet and releases the accounts.
/// The VPS session key is the payer/signer — no wallet popup needed after our auth-check removal.
pub fn undelegate_game_ix(program_id: &Pubkey, session_pubkey: &Pubkey, game_id: u64) -> Instruction {
    let game_pda = Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], program_id).0;
    let move_log_pda = Pubkey::find_program_address(&[MOVE_LOG_SEED, &game_id.to_le_bytes()], program_id).0;
    let magic_context: Pubkey = MAGIC_CONTEXT_PUBKEY.parse().expect("magic context pubkey");
    let magic_program: Pubkey = MAGIC_PROGRAM_PUBKEY.parse().expect("magic program pubkey");

    let mut data = anchor_discriminator("undelegate_game").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());

    Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new(move_log_pda, false),
            AccountMeta::new(*session_pubkey, true),
            AccountMeta::new(magic_context, false),
            AccountMeta::new_readonly(magic_program, false),
        ],
        data,
    }
}

/// Build a `finalize_game` instruction (sent to devnet, after undelegation).
/// Sets game.status = Finished, pays out the wager escrow, and updates ELO.
/// `winner` = Some("white") | Some("black") | None (draw).
///
/// The on-chain `EndGame` accounts struct has no explicit Signer — the session key
/// is only the fee-payer in the outer Transaction, not listed in the instruction accounts.
pub fn finalize_game_ix(
    program_id: &Pubkey,
    game_id: u64,
    white: &Pubkey,
    black: &Pubkey,
    winner: Option<&str>,
) -> Instruction {
    let game_pda = Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], program_id).0;
    let white_profile = Pubkey::find_program_address(&[PROFILE_SEED, white.as_ref()], program_id).0;
    let black_profile = Pubkey::find_program_address(&[PROFILE_SEED, black.as_ref()], program_id).0;
    let escrow_pda = Pubkey::find_program_address(&[WAGER_ESCROW_SEED, &game_id.to_le_bytes()], program_id).0;

    let mut data = anchor_discriminator("finalize_game").to_vec();
    data.extend_from_slice(&game_id.to_le_bytes());

    // GameResult Borsh encoding: 1 = Winner(Pubkey), 2 = Draw
    match winner {
        Some("white") => { data.push(1); data.extend_from_slice(white.as_ref()); }
        Some("black") => { data.push(1); data.extend_from_slice(black.as_ref()); }
        _ =>            { data.push(2); }
    }

    Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new(white_profile, false),
            AccountMeta::new(black_profile, false),
            AccountMeta::new(*white, false),
            AccountMeta::new(*black, false),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
        ],
        data,
    }
}

/// Fund `dest` with `lamports` from `payer`, submit to `rpc_url`.
pub fn fund_account(
    rpc: &RpcClient,
    payer: &Keypair,
    dest: &Pubkey,
    lamports: u64,
) -> Result<Signature> {
    let ix = system_instruction::transfer(&payer.pubkey(), dest, lamports);
    let blockhash = rpc.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[payer], blockhash);
    Ok(rpc.send_and_confirm_transaction(&tx)?)
}

/// Sign `ix` with `signer` (fee-payer = signer) and submit to `rpc_url`.
pub fn sign_and_submit(rpc: &RpcClient, signer: &Keypair, instructions: &[Instruction]) -> Result<Signature> {
    let blockhash = rpc.get_latest_blockhash()?;
    let msg = Message::new(instructions, Some(&signer.pubkey()));
    let tx = Transaction::new(&[signer], msg, blockhash);
    rpc.send_and_confirm_transaction_with_spinner_and_commitment(
        &tx,
        CommitmentConfig::confirmed(),
    )
    .map_err(|e| anyhow!(e))
}

/// Sign and submit to the MagicBlock ER with `skip_preflight = true`.
///
/// The ER's preflight simulator may reject transactions with -32003
/// ("Attempt to load a program that does not exist") because the XFChess
/// program is not in its preflight cache.  The actual ER processing pipeline
/// lazily loads programs from devnet, so skipping preflight lets the TX land.
/// After sending, we poll for confirmation (ER confirms in sub-second to ~5 s).
pub fn sign_and_submit_er(rpc: &RpcClient, signer: &Keypair, instructions: &[Instruction]) -> Result<Signature> {
    use solana_client::rpc_config::RpcSendTransactionConfig;
    use std::time::{Duration, Instant};

    let blockhash = rpc.get_latest_blockhash()?;
    let msg = Message::new(instructions, Some(&signer.pubkey()));
    let tx = Transaction::new(&[signer], msg, blockhash);

    let config = RpcSendTransactionConfig {
        skip_preflight: true,
        ..Default::default()
    };
    let sig = rpc
        .send_transaction_with_config(&tx, config)
        .map_err(|e| anyhow!(e))?;

    let commitment = CommitmentConfig::confirmed();
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if Instant::now() > deadline {
            return Err(anyhow!("ER record_move confirmation timeout for {sig}"));
        }
        match rpc.get_signature_status_with_commitment(&sig, commitment) {
            Ok(Some(Ok(()))) => return Ok(sig),
            Ok(Some(Err(e))) => return Err(anyhow!("ER record_move failed: {e:?}")),
            Ok(None) => std::thread::sleep(Duration::from_millis(400)),
            Err(e) => {
                warn!("[ER] poll error (non-fatal): {e}");
                std::thread::sleep(Duration::from_millis(400));
            }
        }
    }
}

/// Submit an already-signed serialised transaction. Used for wallet-signed setup TXs.
/// Accepts both legacy `Transaction` and `VersionedTransaction` (v0).
/// Uses `confirmed` commitment — much faster than `finalized` (default), reducing
/// the risk of blockhash expiration on devnet before confirmation.
pub fn submit_signed_tx(rpc: &RpcClient, tx_bytes: &[u8]) -> Result<Signature> {
    let tx: VersionedTransaction = bincode::deserialize(tx_bytes).map_err(|e| anyhow!(e))?;
    rpc.send_and_confirm_transaction_with_spinner_and_commitment(
        &tx,
        CommitmentConfig::confirmed(),
    )
    .map_err(|e| anyhow!(e))
}

pub fn make_rpc(url: &str) -> RpcClient {
    RpcClient::new_with_commitment(url.to_string(), CommitmentConfig::confirmed())
}
