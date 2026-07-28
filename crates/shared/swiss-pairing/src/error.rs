use thiserror::Error;

/// Errors that can occur during Swiss pairing
#[derive(Error, Debug)]
pub enum PairingError {
    /// No opponent could be found for `player1`/`player2` under the current constraints.
    #[error("No valid pairing found for players {player1} and {player2}")]
    NoValidPairing {
        /// First player in the failed pairing attempt.
        player1: String,
        /// Second player in the failed pairing attempt.
        player2: String,
    },

    /// An odd-sized pool had no player eligible to receive a bye (e.g. everyone already had one).
    #[error("Odd number of players and no valid bye candidate")]
    NoByeCandidate,

    /// The two players have already played each other and a rematch isn't allowed here.
    #[error("Cannot pair players who have already played: {player1} vs {player2}")]
    AlreadyPlayed {
        /// First player in the rematch.
        player1: String,
        /// Second player in the rematch.
        player2: String,
    },

    /// Referenced a player ID not present in the pool.
    #[error("Player not found: {0}")]
    PlayerNotFound(String),

    /// Requested a round number outside `1..=total_rounds`.
    #[error("Invalid round number: {round}, tournament has {total_rounds} rounds")]
    InvalidRound {
        /// Round number that was requested.
        round: u8,
        /// Total rounds configured for the tournament.
        total_rounds: u8,
    },

    /// Color assignment couldn't satisfy the Dutch system's balance/3-in-a-row rules.
    #[error("Cannot assign colors - color clash detected")]
    ColorClash,

    /// Pairing within a scoregroup failed; carries a description of why.
    #[error("Scoregroup pairing failed: {0}")]
    ScoregroupPairingFailed(String),

    /// All configured rounds have already been played.
    #[error("Tournament complete - all rounds played")]
    TournamentComplete,

    /// Catch-all for invariant violations that shouldn't be reachable in practice.
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Result type for pairing operations
pub type PairingResult<T> = Result<T, PairingError>;
