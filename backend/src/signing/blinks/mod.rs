//! Solana Blinks API for XFChess tournament registration.
//!
//! This module provides the complete Blinks functionality following the Solana Action specification:
//! - Action metadata endpoints (core)
//! - Transaction building for tournament registration (core)
//! - Wallet balance checking (core)
//! - Pre-sign validation and anti-cheat (anti_cheat)
//! - Action chaining for multi-step flows (chains)
//! - Wallet funding integration (funding)
//! - Smart onboarding state machine (onboarding)
//! - PDA calculations (pda)
//! - HTTP route handlers (routes)
//!
//! Blinks allow users to register for tournaments directly from wallet popups
//! without visiting the XFChess website, supporting seamless onboarding.

pub mod anti_cheat;
pub mod chains;
pub mod core;
pub mod funding;
pub mod onboarding;
pub mod pda;
pub mod routes;

// Re-export core types for convenience
pub use core::{
    build_claim_prize_transaction, build_register_transaction, build_start_tournament_transactions,
    check_wallet_balance, get_action_metadata, validate_registration, Action, ActionLinks,
    ActionMetadata, BalanceResult, RegisterTransactionRequest, TransactionResponse,
    ValidationResult,
};

// Re-export routes
pub use routes::blinks_routes;

// PDA seeds - used across multiple submodules
/// PDA seed for tournament accounts
pub const TOURNAMENT_SEED: &[u8] = b"tournament";
/// PDA seed for tournament escrow accounts
pub const TOURNAMENT_ESCROW_SEED: &[u8] = b"tournament_escrow";
/// PDA seed for USDC prize escrow
pub const TOURNAMENT_USDC_PRIZE_SEED: &[u8] = b"t_usdc_prize";
/// PDA seed for player profile accounts
pub const PROFILE_SEED: &[u8] = b"profile";
