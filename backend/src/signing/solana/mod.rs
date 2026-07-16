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

pub mod debug;
pub mod instructions;
pub mod routing;
pub mod rpc;
pub mod telemetry;
pub mod transactions;

pub use debug::{debug_transaction, format_debug_info, parse_program_error, TransactionDebugInfo};
pub use instructions::{
    advance_winner_ix, bracket_position, claim_fees_ix, claim_prize_ix,
    distribute_tournament_prizes_ix, finalize_game_ix, fund_sol_prize_ix, initialize_escrow_ix,
    initialize_match_ix, initialize_shards_ix, initialize_tournament_ix, leave_tournament_ix,
    link_external_elo_ix, record_move_ix, record_result_ix, required_shards, start_tournament_ix,
    undelegate_game_ix, verify_profile_ix, withdraw_treasury_ix,
};
pub use routing::{magic_router_url, route_for_game_write, routed_rpc, TxRoute};
pub use rpc::{fallback_rpc_url, make_rpc, read_with_failover, redact_url, rpc_url_or_devnet};
pub use telemetry::{
    submit_er_with_telemetry, submit_with_telemetry, TxErrorCategory, TxErrorDetail,
};
pub use transactions::{
    cosign_and_submit_tx, fund_account, sign_and_submit, sign_and_submit_er, submit_signed_tx,
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
/// PDA seed for tournament accounts
pub const TOURNAMENT_SEED: &[u8] = b"tournament";

/// MagicBlock magic context account (ER-only)
pub const MAGIC_CONTEXT_PUBKEY: &str = "MagicContext1111111111111111111111111111111";
/// MagicBlock magic program (ER-only)
pub const MAGIC_PROGRAM_PUBKEY: &str = "Magic11111111111111111111111111111111111111";
