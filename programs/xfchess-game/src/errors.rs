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
}

pub use GameErrorCode as XfchessGameError;
