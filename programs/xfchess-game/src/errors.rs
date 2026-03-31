use anchor_lang::prelude::*;

#[error_code]
pub enum GameErrorCode {
    #[msg("Game is already full.")]
    GameAlreadyFull,
    #[msg("Cannot play against yourself.")]
    CannotPlaySelf,
    #[msg("Game is not active.")]
    GameNotActive,
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
    #[msg("Invalid next FEN provided in batch.")]
    InvalidNextFen,
    #[msg("Moves and FENs arrays have different lengths.")]
    InvalidBatchLength,
    #[msg("Batch size exceeds maximum allowed.")]
    BatchTooLarge,
    #[msg("Invalid nonce provided for replay protection.")]
    InvalidNonce,
    #[msg("Game is not in the required status for this operation.")]
    InvalidGameStatus,
    #[msg("Game is not currently disputed.")]
    GameNotDisputed,
    #[msg("Unauthorized to resolve this dispute.")]
    UnauthorizedDisputeResolution,
    #[msg("Wager amount exceeds the maximum allowed.")]
    WagerTooHigh,
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
}

pub use GameErrorCode as XfchessGameError;
