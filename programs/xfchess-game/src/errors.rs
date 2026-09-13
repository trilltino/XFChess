use anchor_lang::prelude::*;

#[error_code]
pub enum GameErrorCode {
    #[msg("Game is already full.")]
    GameAlreadyFull,

    #[msg("Cannot play against yourself.")]
    CannotPlaySelf,

    #[msg("Game is not active.")]
    GameNotActive,

    #[msg("Invalid lifecycle transition.")]
    InvalidLifecycleTransition,

    #[msg("Game is already delegated.")]
    GameAlreadyDelegated,

    #[msg("Game is not delegated.")]
    GameNotDelegated,

    #[msg("Settlement requires the game to be undelegated.")]
    SettlementRequiresUndelegated,

    #[msg("Invalid owner program for delegation.")]
    InvalidOwnerProgram,

    #[msg("Unauthorized relayer.")]
    UnauthorizedRelayer,

    #[msg("Not your turn.")]
    NotPlayerTurn,

    #[msg("Calculation overflow.")]
    Overflow,

    #[msg("You are not in this game.")]
    NotInGame,

    #[msg("Move log is full.")]
    MoveLogFull,

    #[msg("Game has not expired or is not in a withdrawable state.")]
    GameNotExpired,

    #[msg("Only the game creator can withdraw an expired wager.")]
    NotGameCreator,

    #[msg("Missing token accounts for NFT/SPL wager payout.")]
    MissingTokenAccounts,

    #[msg("Wager amount exceeds the maximum allowed.")]
    WagerTooHigh,

    #[msg("Invalid board state or FEN.")]
    InvalidBoardState,

    #[msg("Invalid or illegal chess move.")]
    InvalidMove,

    #[msg("Unauthorized access to this resource.")]
    UnauthorizedAccess,

    #[msg("Invalid session key provided.")]
    InvalidSessionKey,

    #[msg("Session has expired or is disabled.")]
    SessionExpiredOrDisabled,

    #[msg("Session is expired or has been revoked.")]
    SessionExpired,

    #[msg("Session spending limit exceeded.")]
    SessionSpendingLimit,

    #[msg("Session not authorized for this operation.")]
    SessionNotAuthorized,

    #[msg("Wager exceeds session per-match cap.")]
    WagerExceedsSessionCap,

    #[msg("Session spending limit would be exceeded.")]
    SessionSpendingLimitExceeded,

    #[msg("Invalid next FEN provided in batch.")]
    InvalidNextFen,

    #[msg("Moves and FENs arrays have different lengths.")]
    InvalidBatchLength,

    #[msg("Batch size exceeds maximum allowed.")]
    BatchTooLarge,

    #[msg("Invalid nonce provided for replay protection.")]
    InvalidNonce,

    #[msg("Parent nonce mismatch: client's claimed parent state does not match on-chain nonce.")]
    ParentNonceMismatch,

    #[msg("Game is not in the required status for this operation.")]
    InvalidGameStatus,

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

    #[msg("Game is not currently disputed.")]
    GameNotDisputed,

    #[msg("Unauthorized to resolve this dispute.")]
    UnauthorizedDisputeResolution,

    #[msg("Tournament is not in registration phase.")]
    TournamentNotInRegistration,

    #[msg("Tournament is full.")]
    TournamentFull,

    #[msg("Player is already registered for this tournament.")]
    AlreadyRegistered,

    #[msg("Unauthorized: Not the tournament authority.")]
    NotTournamentAuthority,

    #[msg("Invalid tournament match status.")]
    InvalidMatchStatus,

    #[msg("Tournament is not completed.")]
    TournamentNotCompleted,

    #[msg("No prize pool to claim or not the winner.")]
    NoPrizeToClaim,

    #[msg("Tournament is not active.")]
    TournamentNotActive,

    #[msg("Player ELO is below tournament minimum.")]
    EloTooLow,

    #[msg("Player ELO is above tournament maximum.")]
    EloTooHigh,

    #[msg("Player not found in tournament.")]
    PlayerNotFound,

    #[msg("USDC prize pool has not been funded yet.")]
    UsdcPrizeNotFunded,

    #[msg("Guaranteed prize pool must be funded before registration opens.")]
    PrizeNotFunded,

    #[msg("Guaranteed prize pool is already funded and cannot be changed.")]
    PrizeAlreadyFunded,

    #[msg("Minimum player count not reached.")]
    MinPlayersNotReached,

    #[msg("USDC transfer failed.")]
    UsdcTransferFailed,

    #[msg("Insufficient treasury balance for refunds.")]
    InsufficientTreasuryForRefund,

    #[msg("Insufficient funds")]
    InsufficientFunds,
    #[msg("Insufficient prize funds")]
    InsufficientPrizeFunds,

    #[msg("No time limit is set for this game.")]
    NoTimeLimit,

    #[msg("Timeout period has not elapsed yet.")]
    TimeoutNotExpired,

    #[msg("Game is already finished.")]
    AlreadyFinished,

    #[msg("Fee vault claim conditions not yet met (threshold or interval).")]
    FeeVaultNotReady,

    #[msg("Vesting parameters not configured for this tournament.")]
    NoVestingConfigured,

    #[msg("Math overflow in calculation.")]
    MathOverflow,

    #[msg("Not a tournament winner.")]
    NotTournamentWinner,

    #[msg("Cannot close: one or more funded prize places are still unclaimed.")]
    PrizesOutstanding,

    #[msg("Wager amount is below the minimum required")]
    StakeTooLow,
    #[msg("Wager pool is too small to cover the advanced fees")]
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

    #[msg("Invalid username format or length.")]
    InvalidUsername,

    // Username-specific granular errors (moved into the central error
    // enum so Anchor's IDL builder sees a single error definition).
    #[msg("Username must be 3-20 characters")]
    InvalidLength,
    #[msg("Username can only contain A-Z, a-z, 0-9, _, -")]
    InvalidCharacters,
    #[msg("This username is reserved")]
    ReservedUsername,
    #[msg("Username already taken")]
    UsernameTaken,
    #[msg("Username not set")]
    UsernameNotSet,
    #[msg("Cannot change username yet - cooldown active")]
    ChangeCooldown,

    #[msg("Player must be 18 or older to participate in wagered games.")]
    UnderagePlayer,

    #[msg("A valid global session already exists for this player.")]
    GlobalSessionAlreadyActive,
    #[msg("Global session has no games remaining; please re-authorize.")]
    GlobalSessionNoGamesRemaining,
    #[msg("Global session spending limit would be exceeded.")]
    GlobalSessionSpendingLimitExceeded,

    #[msg("Claimed result does not match the on-chain game result.")]
    ResultMismatch,
    #[msg(
        "Game result has not been committed on-chain; cannot finalize a game with no result yet."
    )]
    GameStillInProgress,

    #[msg("Friendship parties must be distinct and passed in canonical (sorted) order.")]
    InvalidFriendPair,
    #[msg("Friend request is not pending.")]
    FriendNotPending,

    #[msg("This board's result has already been recorded for the current round.")]
    BoardAlreadyRecorded,

    #[msg("Not every board has reported a result for the current round yet.")]
    TournamentRoundIncomplete,

    #[msg("Buffer account is not the canonical undelegate-buffer for this account.")]
    InvalidUndelegationBuffer,

    #[msg("Game is not in the post-force-undelegate wiped state.")]
    GameNotStuckDelegation,

    #[msg("complete_swiss_tournament called before every round has been played and advanced.")]
    SwissTournamentNotFinished,

    #[msg("Platform fee exceeds the maximum allowed for a single game.")]
    PlatformFeeTooLarge,

    #[msg("No draw offer is pending for this game.")]
    NoDrawOfferPending,
    #[msg("You cannot accept your own draw offer.")]
    CannotAcceptOwnDrawOffer,

    #[msg("Session vault balance is too low to cover this game's rent and wager. Top it up by re-authorizing the session with a larger deposit.")]
    GlobalSessionVaultUnderfunded,

    #[msg("Crank call arrived too early — game's inactivity window has not elapsed yet.")]
    CrankTooEarly,
    #[msg("Crank call arrived too late — exceeded maximum slot delay from last update.")]
    CrankTooLate,

    #[msg("Account owner does not match the expected program ID.")]
    InvalidAccountOwner,
    #[msg("Session or dispute duration exceeds the maximum allowed.")]
    DurationTooLarge,

    #[msg("Production builds require the `move-validation` feature to be enabled.")]
    ProductionFeatureMissing,

    #[msg("The provided token mint has unsupported extensions for this operation.")]
    UnsupportedMintExtension,

    #[msg("Dynamic remaining accounts do not match expected set, order, or ownership.")]
    InvalidRemainingAccounts,
}

pub use GameErrorCode as XfchessGameError;
