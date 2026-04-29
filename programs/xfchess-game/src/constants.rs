//! Program-wide constants, discriminators, and magic numbers.

use anchor_lang::prelude::*;

// PDA seeds — each is the prefix byte string used to derive a Program Derived Address
// for the corresponding on-chain account type. Changing any seed is a breaking change.

#[constant]
pub const GAME_SEED: &[u8] = b"game"; // Derives the GameState PDA per game_id

#[constant]
pub const MOVE_LOG_SEED: &[u8] = b"move_log"; // Derives the MoveLog PDA that stores move history

#[constant]
pub const PROFILE_SEED: &[u8] = b"profile"; // Derives a player's on-chain profile (ELO, username)

#[constant]
pub const USERNAME_SEED: &[u8] = b"username"; // Derives the uniqueness-lock account for a chosen username

#[constant]
pub const WAGER_ESCROW_SEED: &[u8] = b"escrow"; // Derives the SOL escrow vault that holds wager funds during a game

#[constant]
pub const SESSION_DELEGATION_SEED: &[u8] = b"session_delegation"; // Derives a per-game session-key authorisation record

pub const TOURNAMENT_SEED: &[u8] = b"tournament";       // Derives the TournamentState PDA
pub const TOURNAMENT_ESCROW_SEED: &[u8] = b"t_escrow";  // Derives the prize-pool escrow vault for a tournament
pub const TOURNAMENT_PRIZE_ESCROW_SEED: &[u8] = b"tournament_prize_escrow";  // Derives the prize escrow vault (85% of fees)
pub const TOURNAMENT_OPS_ESCROW_SEED: &[u8] = b"t_ops";      // Derives the ops escrow vault (10% of fees)
pub const TOURNAMENT_OPERATOR_ESCROW_SEED: &[u8] = b"t_operator";  // Derives the operator escrow vault (5% of fees)
pub const TOURNAMENT_MATCH_SEED: &[u8] = b"t_match";    // Derives an individual match record within a tournament
pub const TOURNAMENT_USDC_PRIZE_SEED: &[u8] = b"t_usdc_prize";  // Derives the SPL token escrow for USDC prize pool

// ---------------------------------------------------------------------------
// Privileged authority keypairs
// ---------------------------------------------------------------------------

/// The KYC/identity verification authority (VPS backend signer).
/// Called by `verify_profile` to mark a player as CARF-compliant on-chain.
/// TODO: Replace with a real keypair before mainnet deploy.
pub mod kyc_authority {
    use super::*;
    pub const ID: Pubkey = Pubkey::new_from_array([
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2
    ]);
}

/// The platform dispute-resolution authority — the only signer allowed to
/// call `resolve_dispute`. Set to a secure offline keypair before mainnet.
/// TODO: Replace with a real keypair before mainnet deploy.
pub mod dispute_authority {
    use super::*;
    pub const ID: Pubkey = Pubkey::new_from_array([
        // Replace with actual pubkey for dispute authority
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1
    ]);
}

/// The VPS/backend operational authority — the only signer allowed to call
/// privileged instructions such as `update_elo` and `collect_fee`.
/// Solflare wallet - user's main wallet for signing operations.
pub mod vps_authority {
    use super::*;
    pub const ID: Pubkey = Pubkey::new_from_array([
        // Replace with actual pubkey for VPS authority
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
    ]);
}

/// Hard cap on a single wager so no one can lock more than 10 SOL in one game.
pub const MAX_WAGER_AMOUNT: u64 = 10 * 1_000_000_000; // 10 SOL in lamports

/// Minimum wager amount (0.001 SOL).
pub const MIN_WAGER_LAMPORTS: u64 = 1_000_000;

/// Lamports advanced by each player to cover on-chain compute costs.
pub const CREATE_GAME_COST: u64 = 5_000; // lamports
pub const JOIN_GAME_COST: u64 = 5_000;
pub const DELEGATE_COST: u64 = 5_000;
pub const UNDELEGATE_COST: u64 = 5_000;
pub const COMMIT_ER_COST: u64 = 5_000; // per ER→L1 commit (0 if MagicBlock sponsors)
pub const RECORD_RESULT_COST: u64 = 5_000;
pub const CLAIM_PRIZE_COST: u64 = 5_000;
pub const MARGIN_BPS: u16 = 25; // 0.25% of wager pool, priority-fee spike insurance

// ---------------------------------------------------------------------------
// Regional treasury fees (in lamports)
// ---------------------------------------------------------------------------
// Backend handles local currency conversion, contract uses lamports for on-chain validation

/// UK: 50p GBP per wager/tournament game (backend converts to lamports)
pub const UK_FEE_LAMPORTS: u64 = 50_000_000; // 0.05 SOL (~50p GBP)

/// Brazil: 20p BRL per wager/tournament game (backend converts to lamports)
pub const BRAZIL_FEE_LAMPORTS: u64 = 10_000_000; // 0.01 SOL (~20p BRL)

/// Canada: 40 cents CAD per wager/tournament game (backend converts to lamports)
pub const CANADA_FEE_LAMPORTS: u64 = 40_000_000; // 0.04 SOL (~40c CAD)

/// Germany: 30 cents EUR per wager/tournament game (backend converts to lamports)
pub const GERMANY_FEE_LAMPORTS: u64 = 30_000_000; // 0.03 SOL (~30c EUR)

/// Platform fee per player in lamports (approximately £0.50, assuming 1 SOL ≈ £125, so £0.50 ≈ 0.004 SOL = 4,000,000 lamports)
pub const PLATFORM_FEE_LAMPORTS: u64 = 4_000_000;
pub const PLATFORM_FEE_PERCENT: u64 = 5; // 5% fee on wagers

/// ELO update fee per player
pub const ELO_FEE_LAMPORTS: u64 = 5_000; // 0.000005 SOL per ELO update

/// Treasury vault seed
pub const TREASURY_VAULT_SEED: &[u8] = b"treasury_vault";

/// Time-to-live for an unresolved dispute (7 days in seconds).
/// After this window any party may call claim_stale_dispute for an automatic 50/50 split.
pub const DISPUTE_TTL_SECS: i64 = 604_800;
