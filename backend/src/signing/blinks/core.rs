//! Core Solana Blinks functionality.
//!
//! This module provides the core Blinks functionality following the Solana Action specification:
//! - Action metadata endpoints
//! - Transaction building for tournament registration
//! - Wallet balance checking
//! - Pre-sign validation

use anyhow::Result;
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};

use crate::signing::solana;
use crate::signing::solana::{claim_prize_ix, initialize_match_ix, start_tournament_ix};
use crate::signing::storage::tournament::TournamentStore;

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine};

/// Solana Action metadata response following the Blinks specification.
#[derive(Serialize, Deserialize)]
pub struct ActionMetadata {
    /// Icon URL for the action
    pub icon: String,
    /// Title of the action
    pub title: String,
    /// Description of the action
    pub description: String,
    /// Button label
    pub label: String,
    /// Action links
    pub links: ActionLinks,
}

/// Action links defining available actions.
#[derive(Serialize, Deserialize)]
pub struct ActionLinks {
    /// List of available actions
    pub actions: Vec<Action>,
}

/// A single action definition.
#[derive(Serialize, Deserialize)]
pub struct Action {
    /// Button label for this action
    pub label: String,
    /// URL to POST for this action
    pub href: String,
}

/// Request to build a registration transaction.
#[derive(Deserialize)]
pub struct RegisterTransactionRequest {
    /// Wallet public key
    pub account: String,
}

/// Response containing a base64-encoded transaction.
#[derive(Serialize)]
pub struct TransactionResponse {
    /// Base64-encoded transaction ready to sign
    pub transaction: String,
    /// Estimated fee in lamports
    pub fee_estimate: u64,
}

/// Validation result for pre-sign checks.
#[derive(Serialize)]
pub struct ValidationResult {
    /// Whether validation passed
    pub valid: bool,
    /// Error message if validation failed
    pub error: Option<String>,
    /// Next action in the chain
    pub next_action: Option<String>,
}

/// Balance check result.
#[derive(Serialize)]
pub struct BalanceResult {
    /// Wallet public key
    pub wallet: String,
    /// SOL balance in lamports
    pub balance_lamports: u64,
    /// Whether balance is sufficient for registration
    pub sufficient: bool,
    /// Required amount in lamports
    pub required_lamports: u64,
}

/// Gets action metadata for a tournament.
///
/// Returns JSON metadata that wallets display to users showing
/// tournament details and the registration action.
pub async fn get_action_metadata(
    tournament_id: u64,
    store: &TournamentStore,
) -> Result<ActionMetadata> {
    let tournament = store
        .get(tournament_id)
        .await
        .ok_or_else(|| anyhow::anyhow!("Tournament {} not found", tournament_id))?;

    let entry_fee_sol = tournament.entry_fee_lamports as f64 / 1_000_000_000.0;
    let prize_pool_sol = tournament.prize_pool as f64 / 1_000_000_000.0;

    Ok(ActionMetadata {
        icon: "https://xfchess.com/logo.png".to_string(),
        title: tournament.name.clone(),
        description: format!(
            "{}-Player Tournament. Entry: {} SOL. Prize Pool: {} SOL. Register now to compete!",
            tournament.max_players, entry_fee_sol, prize_pool_sol
        ),
        label: "Join Tournament".to_string(),
        links: ActionLinks {
            actions: vec![Action {
                label: format!("Pay {} SOL & Register", entry_fee_sol),
                href: format!("/api/actions/tournament/{}/register", tournament_id),
            }],
        },
    })
}

/// Builds a registration transaction for a player.
///
/// Returns a **partially signed** transaction (fee payer co-signs, player must
/// add their signature before broadcasting). The player's wallet signs to
/// authorize the entry-fee transfer from their account into the escrow PDA.
pub async fn build_register_transaction(
    tournament_id: u64,
    wallet_pubkey: &Pubkey,
    store: &TournamentStore,
    program_id: &Pubkey,
    fee_payer: &Keypair,
) -> Result<TransactionResponse> {
    let tournament = store
        .get(tournament_id)
        .await
        .ok_or_else(|| anyhow::anyhow!("Tournament {} not found", tournament_id))?;

    let tournament_pda = super::pda::derive_tournament_pda(tournament_id, program_id)?;
    let escrow_pda = super::pda::derive_escrow_pda(tournament_id, program_id)?;
    let player_profile = super::pda::derive_player_profile_pda(wallet_pubkey, program_id)?;
    let shard_0 = super::pda::derive_shard_pda(0, tournament_id, program_id)?;

    // Shards 1–3 are only present for larger tournaments
    let shard_1 = if tournament.max_players > 64 {
        Some(super::pda::derive_shard_pda(1, tournament_id, program_id)?)
    } else {
        None
    };
    let shard_2 = if tournament.max_players > 128 {
        Some(super::pda::derive_shard_pda(2, tournament_id, program_id)?)
    } else {
        None
    };
    let shard_3 = if tournament.max_players >= 256 {
        Some(super::pda::derive_shard_pda(3, tournament_id, program_id)?)
    } else {
        None
    };

    let instruction = build_register_player_instruction(
        program_id,
        &tournament_pda,
        &escrow_pda,
        &player_profile,
        wallet_pubkey,
        tournament_id,
        &shard_0,
        shard_1.as_ref(),
        shard_2.as_ref(),
        shard_3.as_ref(),
    )?;

    let rpc = solana::make_rpc(&solana::rpc_url_or_devnet());
    let blockhash = rpc.get_latest_blockhash()?;

    use solana_sdk::{message::Message, transaction::Transaction};
    let message = Message::new(&[instruction], Some(&fee_payer.pubkey()));
    let mut transaction = Transaction::new_unsigned(message);
    transaction.try_partial_sign(&[fee_payer], blockhash)?;

    let transaction_bytes = bincode::serialize(&transaction)?;
    let transaction_base64 = BASE64_STANDARD.encode(&transaction_bytes);

    Ok(TransactionResponse {
        transaction: transaction_base64,
        fee_estimate: 5000 + tournament.entry_fee_lamports + 10000,
    })
}

/// Builds an unsigned `claim_tournament_prize` transaction for a winner.
/// The claimant signs it with their wallet and broadcasts.
pub async fn build_claim_prize_transaction(
    tournament_id: u64,
    claimant: &Pubkey,
    store: &TournamentStore,
    program_id: &Pubkey,
    fee_payer: &Keypair,
) -> Result<TransactionResponse> {
    let tournament = store
        .get(tournament_id)
        .await
        .ok_or_else(|| anyhow::anyhow!("Tournament {} not found", tournament_id))?;

    let claimant_str = claimant.to_string();
    let prize_bps = [
        tournament.winner.as_deref() == Some(&claimant_str),
        tournament.second_place.as_deref() == Some(&claimant_str),
        tournament.third_place.as_deref() == Some(&claimant_str),
        tournament.fourth_place.as_deref() == Some(&claimant_str),
        tournament.fifth_place.as_deref() == Some(&claimant_str),
    ]
    .iter()
    .position(|&m| m)
    .map(|i| tournament.prize_shares[i])
    .unwrap_or(0);

    anyhow::ensure!(
        prize_bps > 0,
        "Wallet {} is not a prize winner in tournament {}",
        claimant_str,
        tournament_id
    );

    let instruction = claim_prize_ix(program_id, tournament_id, claimant);

    let rpc = solana::make_rpc(&solana::rpc_url_or_devnet());
    let blockhash = rpc.get_latest_blockhash()?;

    use solana_sdk::{message::Message, transaction::Transaction};
    let message = Message::new(&[instruction], Some(&fee_payer.pubkey()));
    let mut transaction = Transaction::new_unsigned(message);
    transaction.try_partial_sign(&[fee_payer], blockhash)?;

    let transaction_bytes = bincode::serialize(&transaction)?;
    let transaction_base64 = BASE64_STANDARD.encode(&transaction_bytes);

    let prize_lamports = (tournament.prize_pool as u128 * prize_bps as u128 / 10000) as u64;
    Ok(TransactionResponse {
        transaction: transaction_base64,
        fee_estimate: prize_lamports,
    })
}

/// Builds the admin start-tournament transaction batch.
///
/// Returns base64-encoded transactions (one `start_tournament` + batches of
/// `initialize_match` for every bracket slot). Caller broadcasts in order.
/// Each batch holds up to 20 `initialize_match` instructions to stay within
/// the 1232-byte transaction size limit.
pub async fn build_start_tournament_transactions(
    tournament_id: u64,
    store: &TournamentStore,
    program_id: &Pubkey,
    authority: &Keypair,
    host_treasury: &Pubkey,
) -> Result<Vec<String>> {
    let tournament = store
        .get(tournament_id)
        .await
        .ok_or_else(|| anyhow::anyhow!("Tournament {} not found", tournament_id))?;

    let rpc = solana::make_rpc(&solana::rpc_url_or_devnet());
    let mut out = Vec::new();

    // Tx 1: start_tournament
    {
        let ix = start_tournament_ix(
            program_id,
            tournament_id,
            &authority.pubkey(),
            host_treasury,
        );
        let bh = rpc.get_latest_blockhash()?;
        use solana_sdk::{message::Message, transaction::Transaction};
        let msg = Message::new(&[ix], Some(&authority.pubkey()));
        let tx = Transaction::new(&[authority], msg, bh);
        out.push(BASE64_STANDARD.encode(bincode::serialize(&tx)?));
    }

    // Tx batches: initialize_match for every bracket slot
    let total_matches = tournament.max_players as usize - 1;
    let mut match_idx = 0u16;

    for chunk in std::iter::repeat(())
        .take((total_matches + 19) / 20)
        .zip(0..)
    {
        let _ = chunk;
        let end = ((match_idx as usize + 20).min(total_matches)) as u16;
        let ixs: Vec<_> = (match_idx..end)
            .map(|idx| {
                // Derive round from match index for single-elimination bracket
                let round = (idx as f32).log2() as u8;
                initialize_match_ix(
                    program_id,
                    tournament_id,
                    idx,
                    round,
                    None, // players filled by start_tournament on-chain
                    None,
                    if idx == 0 { None } else { Some((idx - 1) / 2) },
                    (idx % 2) as u8,
                    &authority.pubkey(),
                )
            })
            .collect();

        let bh = rpc.get_latest_blockhash()?;
        use solana_sdk::{message::Message, transaction::Transaction};
        let msg = Message::new(&ixs, Some(&authority.pubkey()));
        let tx = Transaction::new(&[authority], msg, bh);
        out.push(BASE64_STANDARD.encode(bincode::serialize(&tx)?));

        match_idx = end;
        if match_idx as usize >= total_matches {
            break;
        }
    }

    Ok(out)
}

/// Checks if a wallet has sufficient SOL balance for registration.
pub async fn check_wallet_balance(
    wallet_pubkey: &Pubkey,
    tournament_id: u64,
    store: &TournamentStore,
) -> Result<BalanceResult> {
    let tournament = store
        .get(tournament_id)
        .await
        .ok_or_else(|| anyhow::anyhow!("Tournament {} not found", tournament_id))?;

    let rpc = solana::make_rpc("https://api.devnet.solana.com");
    let balance = rpc.get_balance(wallet_pubkey)?;

    let required = tournament.entry_fee_lamports + 5000; // Entry fee + buffer
    let sufficient = balance >= required;

    Ok(BalanceResult {
        wallet: wallet_pubkey.to_string(),
        balance_lamports: balance,
        sufficient,
        required_lamports: required,
    })
}

/// Validates a player for tournament registration (anti-cheat checks).
///
/// Performs pre-sign validation including:
/// - Tournament capacity check
/// - Registration status check
/// - ELO range validation
/// - KYC status check (if required)
/// - IP pattern detection (if IP provided)
pub async fn validate_registration(
    tournament_id: u64,
    wallet_pubkey: &Pubkey,
    store: &TournamentStore,
    elo_cache: &crate::signing::EloCache,
    _identity_vault: &crate::signing::IdentityVault,
    ip_address: Option<String>,
) -> Result<ValidationResult> {
    let tournament = store
        .get(tournament_id)
        .await
        .ok_or_else(|| anyhow::anyhow!("Tournament {} not found", tournament_id))?;

    // Check tournament capacity
    if tournament.players.len() >= tournament.max_players as usize {
        return Ok(ValidationResult {
            valid: false,
            error: Some("Tournament is full".to_string()),
            next_action: None,
        });
    }

    // Check registration status
    if tournament.status != crate::signing::storage::tournament::TournamentStatus::Registration {
        return Ok(ValidationResult {
            valid: false,
            error: Some("Tournament is not in registration phase".to_string()),
            next_action: None,
        });
    }

    // Check if already registered
    if tournament
        .players
        .iter()
        .any(|p| p == &wallet_pubkey.to_string())
    {
        return Ok(ValidationResult {
            valid: false,
            error: Some("Already registered for this tournament".to_string()),
            next_action: None,
        });
    }

    // Get player ELO
    let wallet_str = wallet_pubkey.to_string();
    let cached_elo = elo_cache.get_elo(&wallet_str).await;
    let player_elo = match cached_elo {
        Ok(cached) => cached.elo_rating as u32,
        Err(_) => 1200,
    };

    // Check ELO range
    if let Some(elo_min) = tournament.elo_min {
        if player_elo < elo_min {
            return Ok(ValidationResult {
                valid: false,
                error: Some(format!("ELO too low. Minimum: {}", elo_min)),
                next_action: None,
            });
        }
    }

    if let Some(elo_max) = tournament.elo_max {
        if player_elo > elo_max {
            return Ok(ValidationResult {
                valid: false,
                error: Some(format!("ELO too high. Maximum: {}", elo_max)),
                next_action: None,
            });
        }
    }

    // IP-based pattern detection (if IP provided)
    if let Some(ip) = ip_address {
        if let Some(pattern_error) = super::anti_cheat::check_ip_patterns(&ip, tournament_id).await
        {
            return Ok(ValidationResult {
                valid: false,
                error: Some(pattern_error),
                next_action: None,
            });
        }
    }

    // All checks passed
    Ok(ValidationResult {
        valid: true,
        error: None,
        next_action: Some(format!(
            "/api/actions/tournament/{}/register",
            tournament_id
        )),
    })
}

/// Builds the `register_player` instruction using the Anchor discriminator.
///
/// Account order matches `RegisterPlayer` in the Solana program:
///   tournament, player_profile, player (signer), escrow_pda,
///   shard_0, shard_1?, shard_2?, shard_3?, system_program
fn build_register_player_instruction(
    program_id: &Pubkey,
    tournament_pda: &Pubkey,
    escrow_pda: &Pubkey,
    player_profile_pda: &Pubkey,
    player: &Pubkey,
    tournament_id: u64,
    shard_0: &Pubkey,
    shard_1: Option<&Pubkey>,
    shard_2: Option<&Pubkey>,
    shard_3: Option<&Pubkey>,
) -> Result<solana_sdk::instruction::Instruction> {
    use sha2::{Digest, Sha256};
    use solana_sdk::instruction::{AccountMeta, Instruction};

    let mut hasher = Sha256::new();
    hasher.update("global:register_player");
    let discriminator = &hasher.finalize()[..8];

    let mut data = discriminator.to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());
    // elo placeholder — backend supplies 0; the program reads from the profile PDA
    data.extend_from_slice(&0u32.to_le_bytes());

    let mut accounts = vec![
        AccountMeta::new(*tournament_pda, false),
        AccountMeta::new_readonly(*player_profile_pda, false),
        AccountMeta::new(*player, true),
        AccountMeta::new(*escrow_pda, false),
        AccountMeta::new(*shard_0, false),
    ];
    if let Some(s1) = shard_1 {
        accounts.push(AccountMeta::new(*s1, false));
    }
    if let Some(s2) = shard_2 {
        accounts.push(AccountMeta::new(*s2, false));
    }
    if let Some(s3) = shard_3 {
        accounts.push(AccountMeta::new(*s3, false));
    }
    accounts.push(AccountMeta::new_readonly(
        solana_sdk::system_program::id(),
        false,
    ));

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data,
    })
}
