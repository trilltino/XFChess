use thiserror::Error;

/// Errors that can occur during Swiss pairing
#[derive(Error, Debug)]
pub enum PairingError {
    #[error("No valid pairing found for players {player1} and {player2}")]
    NoValidPairing { player1: String, player2: String },

    #[error("Odd number of players and no valid bye candidate")]
    NoByeCandidate,

    #[error("Cannot pair players who have already played: {player1} vs {player2}")]
    AlreadyPlayed { player1: String, player2: String },

    #[error("Player not found: {0}")]
    PlayerNotFound(String),

    #[error("Invalid round number: {round}, tournament has {total_rounds} rounds")]
    InvalidRound { round: u8, total_rounds: u8 },

    #[error("Cannot assign colors - color clash detected")]
    ColorClash,

    #[error("Scoregroup pairing failed: {0}")]
    ScoregroupPairingFailed(String),

    #[error("Tournament complete - all rounds played")]
    TournamentComplete,

    #[error("Internal error: {0}")]
    Internal(String),
}

/// Result type for pairing operations
pub type PairingResult<T> = Result<T, PairingError>;
