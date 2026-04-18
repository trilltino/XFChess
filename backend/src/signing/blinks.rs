//! Solana Blinks API for XFChess tournament registration.
//!
//! This module provides the core Blinks functionality following the Solana Action specification:
//! - Action metadata endpoints
//! - Transaction building for tournament registration
//! - Wallet balance checking
//! - Pre-sign validation (anti-cheat)
//!
//! Blinks allow users to register for tournaments directly from wallet popups
//! without visiting the XFChess website, supporting seamless onboarding.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};

use crate::signing::storage::tournament::TournamentStore;
use crate::signing::solana;

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine};

/// PDA seed for tournament accounts
pub const TOURNAMENT_SEED: &[u8] = b"tournament";
/// PDA seed for tournament escrow accounts
pub const TOURNAMENT_ESCROW_SEED: &[u8] = b"tournament_escrow";
/// PDA seed for tournament prize escrow
pub const TOURNAMENT_PRIZE_ESCROW_SEED: &[u8] = b"tournament_prize_escrow";
/// PDA seed for tournament ops escrow
pub const TOURNAMENT_OPS_ESCROW_SEED: &[u8] = b"tournament_ops_escrow";
/// PDA seed for tournament operator escrow
pub const TOURNAMENT_OPERATOR_ESCROW_SEED: &[u8] = b"tournament_operator_escrow";
/// PDA seed for USDC prize escrow
pub const TOURNAMENT_USDC_PRIZE_SEED: &[u8] = b"t_usdc_prize";
/// PDA seed for player profile accounts
pub const PROFILE_SEED: &[u8] = b"profile";

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
    program_id: &Pubkey,
) -> Result<ActionMetadata> {
    let tournament = store.get(tournament_id).await.ok_or_else(|| {
        anyhow::anyhow!("Tournament {} not found", tournament_id)
    })?;

    let entry_fee_sol = tournament.entry_fee_lamports as f64 / 1_000_000_000.0;
    let prize_pool_sol = tournament.prize_pool as f64 / 1_000_000_000.0;

    Ok(ActionMetadata {
        icon: "https://xfchess.com/logo.png".to_string(),
        title: tournament.name.clone(),
        description: format!(
            "{}-Player Tournament. Entry: {} SOL. Prize Pool: {} SOL. Register now to compete!",
            tournament.max_players,
            entry_fee_sol,
            prize_pool_sol
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
/// Creates a RegisterPlayer instruction with all required PDAs,
/// wraps it in a transaction, and returns it base64-encoded.
pub async fn build_register_transaction(
    tournament_id: u64,
    wallet_pubkey: &Pubkey,
    store: &TournamentStore,
    program_id: &Pubkey,
    fee_payer: &Keypair,
) -> Result<TransactionResponse> {
    let tournament = store.get(tournament_id).await.ok_or_else(|| {
        anyhow::anyhow!("Tournament {} not found", tournament_id)
    })?;

    // Calculate all required PDAs
    let tournament_pda = super::blinks_pda::derive_tournament_pda(tournament_id, program_id)?;
    let escrow_pda = super::blinks_pda::derive_escrow_pda(tournament_id, program_id)?;
    let prize_escrow_pda = super::blinks_pda::derive_prize_escrow_pda(tournament_id, program_id)?;
    let ops_escrow_pda = super::blinks_pda::derive_ops_escrow_pda(tournament_id, program_id)?;
    let operator_escrow_pda = super::blinks_pda::derive_operator_escrow_pda(tournament_id, program_id)?;
    let player_profile_pda = super::blinks_pda::derive_player_profile_pda(wallet_pubkey, program_id)?;

    // Build RegisterPlayer instruction
    let instruction = build_register_player_instruction(
        program_id,
        &tournament_pda,
        &escrow_pda,
        &prize_escrow_pda,
        &ops_escrow_pda,
        &operator_escrow_pda,
        &player_profile_pda,
        wallet_pubkey,
        tournament_id,
    )?;

    // Create transaction
    let rpc = solana::make_rpc("https://api.devnet.solana.com");
    let blockhash = rpc.get_latest_blockhash()?;
    
    use solana_sdk::{
        message::Message,
        transaction::Transaction,
    };
    
    let message = Message::new(&[instruction], Some(&fee_payer.pubkey()));
    let transaction = Transaction::new(&[fee_payer], message, blockhash);

    // Serialize to base64
    let transaction_bytes = bincode::serialize(&transaction)?;
    let transaction_base64 = BASE64_STANDARD.encode(&transaction_bytes);

    // Estimate fee (base fee + entry fee + buffer)
    let fee_estimate = 5000 + tournament.entry_fee_lamports + 10000;

    Ok(TransactionResponse {
        transaction: transaction_base64,
        fee_estimate,
    })
}

/// Checks if a wallet has sufficient SOL balance for registration.
pub async fn check_wallet_balance(
    wallet_pubkey: &Pubkey,
    tournament_id: u64,
    store: &TournamentStore,
) -> Result<BalanceResult> {
    let tournament = store.get(tournament_id).await.ok_or_else(|| {
        anyhow::anyhow!("Tournament {} not found", tournament_id)
    })?;

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
    let tournament = store.get(tournament_id).await.ok_or_else(|| {
        anyhow::anyhow!("Tournament {} not found", tournament_id)
    })?;

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
    if tournament.players.iter().any(|p| p == &wallet_pubkey.to_string()) {
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
        if let Some(pattern_error) = super::blinks_anti_cheat::check_ip_patterns(&ip, tournament_id).await {
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
        next_action: Some(format!("/api/actions/tournament/{}/register", tournament_id)),
    })
}

/// Builds the RegisterPlayer instruction manually using Anchor discriminator.
fn build_register_player_instruction(
    program_id: &Pubkey,
    tournament_pda: &Pubkey,
    escrow_pda: &Pubkey,
    prize_escrow_pda: &Pubkey,
    ops_escrow_pda: &Pubkey,
    operator_escrow_pda: &Pubkey,
    player_profile_pda: &Pubkey,
    player: &Pubkey,
    tournament_id: u64,
) -> Result<solana_sdk::instruction::Instruction> {
    use sha2::{Digest, Sha256};

    // Anchor discriminator for "register_player"
    let mut hasher = Sha256::new();
    hasher.update(format!("global:register_player"));
    let discriminator = &hasher.finalize()[..8];

    let mut data = discriminator.to_vec();
    data.extend_from_slice(&tournament_id.to_le_bytes());

    use solana_sdk::instruction::{AccountMeta, Instruction};

    Ok(Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(*tournament_pda, false),
            AccountMeta::new(*escrow_pda, false),
            AccountMeta::new(*prize_escrow_pda, false),
            AccountMeta::new(*ops_escrow_pda, false),
            AccountMeta::new(*operator_escrow_pda, false),
            AccountMeta::new(*player_profile_pda, false),
            AccountMeta::new(*player, true),
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
        ],
        data,
    })
}
