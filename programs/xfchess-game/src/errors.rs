//! Custom error codes and definitions used across the program.

use anchor_lang::prelude::*;

/// All on-chain errors the program can return.
/// Anchor maps each variant to a u32 error code starting at 6000.
/// The `#[msg]` text is what clients (and logs) see.
#[error_code]
pub enum GameErrorCode {
    // ── Game lifecycle ─────────────────────────────────────────────────────────
    #[msg("Game is already full.")]
    GameAlreadyFull,         // Both player slots occupied when join_game is called

    #[msg("Cannot play against yourself.")]
    CannotPlaySelf,          // Challenger pubkey matches the creator's pubkey

    #[msg("Game is not active.")]
    GameNotActive,           // Instruction requires GameStatus::Active but it isn't

    #[msg("Not your turn.")]
    NotPlayerTurn,           // The signer is not the player whose turn it is

    #[msg("Calculation overflow.")]
    Overflow,                // Arithmetic overflow in wager/ELO math

    #[msg("You are not in this game.")]
    NotInGame,               // Signer is neither white nor black in this game

    #[msg("Move log is full.")]
    MoveLogFull,             // MoveLog account hit its max capacity of stored moves

    // ── Wager / escrow ─────────────────────────────────────────────────────────
    #[msg("Game has not expired or is not in a withdrawable state.")]
    GameNotExpired,          // Tried to reclaim wager before the game timeout elapsed

    #[msg("Only the game creator can withdraw an expired wager.")]
    NotGameCreator,          // Non-creator attempted to reclaim an expired wager

    #[msg("Missing token accounts for NFT/SPL wager payout.")]
    MissingTokenAccounts,    // SPL/NFT payout path called without providing token accounts

    #[msg("Wager amount exceeds the maximum allowed.")]
    WagerTooHigh,            // Wager > MAX_WAGER_AMOUNT (10 SOL)

    // ── Move validation ────────────────────────────────────────────────────────
    #[msg("Invalid board state or FEN.")]
    InvalidBoardState,       // Provided FEN string cannot be parsed as a valid chess position

    #[msg("Invalid or illegal chess move.")]
    InvalidMove,             // Move string is not legal in the current position

    // ── Session / delegation ───────────────────────────────────────────────────
    #[msg("Unauthorized access to this resource.")]
    UnauthorizedAccess,      // Signer is not the expected authority for this instruction

    #[msg("Invalid session key provided.")]
    InvalidSessionKey,       // Session key doesn't match the one stored on-chain

    #[msg("Session has expired or is disabled.")]
    SessionExpiredOrDisabled, // Session was revoked or its timestamp has passed

    #[msg("Session is expired or has been revoked.")]
    SessionExpired,          // Used specifically for the per-player SessionToken check

    #[msg("Session spending limit exceeded.")]
    SessionSpendingLimit,    // Wager would exceed the session's configured spending cap

    #[msg("Session not authorized for this operation.")]
    SessionNotAuthorized,    // Session key mismatch or session disabled

    #[msg("Wager exceeds session per-match cap.")]
    WagerExceedsSessionCap,  // Wager > session.max_wager

    #[msg("Session spending limit would be exceeded.")]
    SessionSpendingLimitExceeded, // total_spent + new_cost > spending_limit

    // ── Batch moves ───────────────────────────────────────────────────────────
    #[msg("Invalid next FEN provided in batch.")]
    InvalidNextFen,          // A FEN in a commit_move_batch call is malformed

    #[msg("Moves and FENs arrays have different lengths.")]
    InvalidBatchLength,      // moves.len() != next_fens.len() in a batch call

    #[msg("Batch size exceeds maximum allowed.")]
    BatchTooLarge,           // Batch commit would exceed the per-transaction limit

    #[msg("Invalid nonce provided for replay protection.")]
    InvalidNonce,            // Nonce is not strictly incrementing — replay attack guard

    // ── Game status ───────────────────────────────────────────────────────────
    #[msg("Game is not in the required status for this operation.")]
    InvalidGameStatus,       // Generic status mismatch (e.g. trying to end a finished game)

    #[msg("Game is not finished")]
    GameNotFinished,
    #[msg("Invalid winner specified")]
    InvalidWinner,
    #[msg("Not your turn to move")]
    NotYourTurn,
    #[msg("Duplicate player account detected")]
    DuplicatePlayerAccount,
    #[msg("Missing player account")]
    MissingPlayerAccount,
    #[msg("Invalid player account")]
    InvalidPlayerAccount,
    #[msg("Prize already claimed")]
    PrizeAlreadyClaimed,
    #[msg("Invalid mint for USDC")]
    InvalidMint,

    // ── Disputes ──────────────────────────────────────────────────────────────
    #[msg("Game is not currently disputed.")]
    GameNotDisputed,         // resolve_dispute called on a game with no open dispute

    #[msg("Unauthorized to resolve this dispute.")]
    UnauthorizedDisputeResolution, // Signer is not the program's designated dispute authority

    // ── Tournament ────────────────────────────────────────────────────────────
    #[msg("Tournament is not in registration phase.")]
    TournamentNotInRegistration, // register_player called after registration has closed

    #[msg("Tournament is full.")]
    TournamentFull,          // Participant cap reached

    #[msg("Player is already registered for this tournament.")]
    AlreadyRegistered,       // Duplicate registration attempt

    #[msg("Unauthorized: Not the tournament authority.")]
    NotTournamentAuthority,  // Caller is not the tournament creator / admin

    #[msg("Invalid tournament match status.")]
    InvalidMatchStatus,      // Match state machine transition is illegal

    #[msg("Tournament is not completed.")]
    TournamentNotCompleted,  // claim_tournament_prize called before the final is resolved

    #[msg("No prize pool to claim or not the winner.")]
    NoPrizeToClaim,          // Caller is not the tournament winner, or prize already claimed

    #[msg("Tournament is not active.")]
    TournamentNotActive,     // Instruction requires TournamentStatus::Active

    #[msg("Player ELO is below tournament minimum.")]
    EloTooLow,               // Player's ELO rating is less than tournament.elo_min

    #[msg("Player ELO is above tournament maximum.")]
    EloTooHigh,              // Player's ELO rating is greater than tournament.elo_max

    #[msg("Player not found in tournament.")]
    PlayerNotFound,          // Player is not registered in the tournament

    #[msg("USDC prize pool has not been funded yet.")]
    UsdcPrizeNotFunded,      // register_player called before operator deposited USDC prize

    #[msg("USDC transfer failed.")]
    UsdcTransferFailed,      // SPL token transfer failed (insufficient balance or approval)

    #[msg("Insufficient treasury balance for refunds.")]
    InsufficientTreasuryForRefund, // host_treasury doesn't have enough SOL to refund players on cancel

    #[msg("Insufficient funds")]
    InsufficientFunds,
    #[msg("Insufficient prize funds")]
    InsufficientPrizeFunds,

    // ── Timeout / resign ──────────────────────────────────────────────────────
    #[msg("No time limit is set for this game.")]
    NoTimeLimit,             // claim_timeout called on a game with base_time_seconds == 0

    #[msg("Timeout period has not elapsed yet.")]
    TimeoutNotExpired,       // claim_timeout called before the move timer has run out

    #[msg("Game is already finished.")]
    AlreadyFinished,         // Instruction requires an active game but it is already done

    // ── Fee vault ─────────────────────────────────────────────────────────────
    #[msg("Fee vault claim conditions not yet met (threshold or interval).")]
    FeeVaultNotReady,        // claim_fees called before min balance/time conditions are met

    #[msg("Vesting parameters not configured for this tournament.")]
    NoVestingConfigured,     // Streaming claim attempted on tournament without vesting params

    #[msg("Math overflow in calculation.")]
    MathOverflow,           // Safe math overflow check failed

    #[msg("Not a tournament winner.")]
    NotTournamentWinner,    // Caller is not in winner list for prize claim

    // ── Fee rebate system ─────────────────────────────────────────────────────
    #[msg("Wager amount is below the minimum required")]
    StakeTooLow,
    #[msg("Wager pool is too small to cover fees and margin")]
    PoolTooSmallForFees,
    #[msg("Fee payer does not match the initial payer")]
    FeePayerMismatch,
    #[msg("Arithmetic overflow occurred")]
    ArithmeticOverflow,

    #[msg("Invalid argument")]
    InvalidArgument,

    #[msg("ELO is out of range")]
    EloOutOfRange,
    #[msg("Invalid session")]
    InvalidSession,
    #[msg("Spending limit exceeded")]
    SpendingLimitExceeded,
    #[msg("Wager limit exceeded")]
    WagerLimitExceeded,
    #[msg("Invalid tournament status")]
    InvalidTournamentStatus,
}

// Alias so the rest of the codebase can use either name.
pub use GameErrorCode as XfchessGameError;
