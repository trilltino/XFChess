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
pub const TOURNAMENT_MATCH_SEED: &[u8] = b"t_match";    // Derives an individual match record within a tournament

// ---------------------------------------------------------------------------
// Privileged authority keypairs
// ---------------------------------------------------------------------------

/// The AI signer that can autonomously record moves on behalf of a player
/// during AI-assisted games. Replaced at deploy time with a real keypair.
pub mod ai_authority {
    use anchor_lang::prelude::declare_id;
    declare_id!("AJwEwo74nRiZ3MPKX3XRh92rJaHj5ktPGRiY8kXhVozp");
}

/// The KYC/identity verification authority (VPS backend signer).
/// Called by `verify_profile` to mark a player as CARF-compliant on-chain.
pub mod kyc_authority {
    use anchor_lang::prelude::declare_id;
    declare_id!("KYCxxZ6UqUeL8XbU7rQfT6YpZ7ZpU7rQfT6YpZ7ZpU7"); // Mock — replace before mainnet
}

/// Hard cap on a single wager so no one can lock more than 10 SOL in one game.
pub const MAX_WAGER_AMOUNT: u64 = 10 * 1_000_000_000; // 10 SOL in lamports

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

/// ELO update fee per player
pub const ELO_FEE_LAMPORTS: u64 = 5_000; // 0.000005 SOL per ELO update

/// Treasury vault seed
pub const TREASURY_VAULT_SEED: &[u8] = b"treasury_vault";
