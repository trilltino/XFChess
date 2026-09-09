pub mod debug;
pub mod game_account;
pub mod game_participants;
pub mod instructions;
pub mod routing;
pub mod rpc;
pub mod telemetry;
pub mod transactions;
pub mod tx_guard;

pub use debug::{debug_transaction, format_debug_info, parse_program_error, TransactionDebugInfo};
pub use instructions::{
    advance_round_ix, advance_winner_ix, bracket_position, cancel_time_check_ix,
    cancel_tournament_ix, claim_prize_ix, complete_swiss_tournament_ix, delegate_game_ix,
    distribute_tournament_prizes_ix, finalize_game_ix, force_undelegate_after_timeout_ix,
    fund_sol_prize_ix, global_record_move_ix, initialize_escrow_ix, initialize_match_ix,
    initialize_shards_ix, initialize_tournament_ix, leave_tournament_ix, link_external_elo_ix,
    record_move_ix, record_result_ix, record_swiss_result_ix, recover_stuck_delegation_ix,
    register_player_ix, request_force_undelegate_ix, required_shards, schedule_time_check_ix,
    start_tournament_ix, undelegate_game_ix, verify_profile_ix, withdraw_treasury_ix,
};
pub use routing::{rpc_for, rpc_url_for, Instr as RoutedInstr, Layer as RpcLayer};
pub use rpc::{
    fallback_rpc_url, make_rpc, read_with_failover, redact_url, rpc_url_or_devnet,
    wallet_signable_blockhash,
};
pub use telemetry::{
    classify_error_str, submit_er_with_telemetry, submit_with_telemetry, TxErrorCategory,
    TxErrorDetail,
};
pub use transactions::{
    cosign_and_submit_tx, fund_account, sign_and_submit, sign_and_submit_er, submit_signed_tx,
};
pub use tx_guard::validate_cosignable_tx;

pub const GAME_SEED: &[u8] = b"game";
pub const MOVE_LOG_SEED: &[u8] = b"move_log";
pub const SESSION_DELEGATION_SEED: &[u8] = b"session_delegation";
pub const PROFILE_SEED: &[u8] = b"profile";
pub const WAGER_ESCROW_SEED: &[u8] = b"escrow";
pub const TOURNAMENT_SEED: &[u8] = b"tournament";
pub const TREASURY_VAULT_SEED: &[u8] = b"treasury_vault";

pub const MAGIC_CONTEXT_PUBKEY: &str = "MagicContext1111111111111111111111111111111";
pub const MAGIC_PROGRAM_PUBKEY: &str = "Magic11111111111111111111111111111111111111";
pub const DELEGATION_PROGRAM_ID: &str = "DELeGGvXpWV2fqJUhqcF5ZSYMS4JTLjteaAMARRSaeSh";
