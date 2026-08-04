//! Error type for the anti-cheat analysis pipeline.

use thiserror::Error;

/// Failure modes for `analyse_game` and the surrounding pipeline.
#[derive(Debug, Error)]
pub enum AcError {
    /// The Stockfish subprocess exited unexpectedly or wrote something the UCI parser rejected.
    #[error("stockfish process error: {0}")]
    Stockfish(String),
    /// Stockfish didn't respond within the configured movetime budget.
    #[error("stockfish timed out after {0}ms")]
    StockfishTimeout(u64),
    /// A line from the Stockfish process didn't parse as valid UCI output.
    #[error("UCI parse error: {0}")]
    UciParse(String),
    /// The game is too short to produce a meaningful signal (game_id, ply_count).
    #[error("not enough moves to analyse (game_id={0}, ply_count={1})")]
    InsufficientMoves(String, usize),
    /// Persisting or loading analysis state failed.
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    /// The analysis worker queue is at capacity; caller should retry later.
    #[error("queue full — try again later")]
    QueueFull,
}

/// Result alias used throughout the crate.
pub type AcResult<T> = Result<T, AcError>;
