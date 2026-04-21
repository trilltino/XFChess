use thiserror::Error;

#[derive(Error, Debug)]
pub enum MultiplayerError {
    #[error("Network error: {0}")]
    Network(String),
    
    #[error("P2P error: {0}")]
    P2P(String),
    
    #[error("Solana error: {0}")]
    Solana(String),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Braid error: {0}")]
    Braid(String),
    
    #[error("VPS error: {0}")]
    Vps(String),
    
    #[error("Session error: {0}")]
    Session(String),
}

pub type MultiplayerResult<T> = Result<T, MultiplayerError>;
