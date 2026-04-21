//! Solana instruction builders and RPC helpers for the XFChess program.
//!
//! This module provides functions to build Solana instructions for:
//! - Recording chess moves on the Execution Rollup
//! - Undelegating games (committing ER state back to devnet)
//! - Finalizing games (setting winner, paying out escrow)
//! - Verifying player profiles (KYC)
//!
//! Also provides RPC client helpers for signing and submitting transactions
//! to both devnet and the MagicBlock Execution Rollup.

pub mod instructions;
pub mod rpc;
pub mod transactions;

pub use instructions::{
    claim_fees_ix, finalize_game_ix, record_move_ix, undelegate_game_ix, verify_profile_ix,
};
pub use rpc::make_rpc;
pub use transactions::{
    fund_account, sign_and_submit, sign_and_submit_er, submit_signed_tx,
};

/// PDA seed for game accounts
pub const GAME_SEED: &[u8] = b"game";
/// PDA seed for move log accounts
pub const MOVE_LOG_SEED: &[u8] = b"move_log";
/// PDA seed for session delegation accounts
pub const SESSION_DELEGATION_SEED: &[u8] = b"session_delegation";
/// PDA seed for player profile accounts
pub const PROFILE_SEED: &[u8] = b"profile";
/// PDA seed for wager escrow accounts
pub const WAGER_ESCROW_SEED: &[u8] = b"escrow";
/// PDA seed for platform fee vault
pub const PLATFORM_FEE_VAULT_SEED: &[u8] = b"platform_fee_vault";

/// MagicBlock magic context account (ER-only)
pub const MAGIC_CONTEXT_PUBKEY: &str = "MagicContext1111111111111111111111111111111";
/// MagicBlock magic program (ER-only)
pub const MAGIC_PROGRAM_PUBKEY: &str = "Magic11111111111111111111111111111111111111";
