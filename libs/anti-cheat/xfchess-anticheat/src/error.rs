use thiserror::Error;

#[derive(Debug, Error)]
pub enum AcError {
    #[error("stockfish process error: {0}")]
    Stockfish(String),
    #[error("stockfish timed out after {0}ms")]
    StockfishTimeout(u64),
    #[error("UCI parse error: {0}")]
    UciParse(String),
    #[error("not enough moves to analyse (game_id={0}, ply_count={1})")]
    InsufficientMoves(String, usize),
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("queue full — try again later")]
    QueueFull,
}

pub type AcResult<T> = Result<T, AcError>;
