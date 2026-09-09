

#[derive(Debug, thiserror::Error)]
pub enum GameError {
    #[error("Invalid move: {message}")]
    InvalidMove { message: String },

    #[error("Engine synchronization failed: {message}")]
    EngineSync { message: String },

    #[error("Piece not found at position ({x}, {y})")]
    PieceNotFound { x: u8, y: u8 },

    #[error("Invalid game state transition: {message}")]
    InvalidStateTransition { message: String },

    #[error("Required resource not initialized: {resource_name}")]
    ResourceNotInitialized { resource_name: String },
}

pub type GameResult<T> = Result<T, GameError>;
