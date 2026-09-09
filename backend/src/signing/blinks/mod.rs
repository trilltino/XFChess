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
pub const TOURNAMENT_SEED: &[u8] = b"tournament";
pub const TOURNAMENT_ESCROW_SEED: &[u8] = b"tournament_escrow";
pub const TOURNAMENT_USDC_PRIZE_SEED: &[u8] = b"t_usdc_prize";
pub const PROFILE_SEED: &[u8] = b"profile";
