//! Web-to-Native Session Bridge
//!
//! This module handles receiving session data from the web app via
//! environment variables or temp files. The session allows the native
//! game to sign transactions on behalf of the user's wallet using
//! an ephemeral session keypair.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer};
use std::path::PathBuf;

/// Session data passed from web app to native game
#[derive(Debug, Clone, Serialize, Deserialize, Resource)]
pub struct GameSession {
    /// The wallet public key that owns this session
    pub wallet_pubkey: Pubkey,

    /// The session signer public key (ephemeral keypair)
    pub session_signer: Pubkey,

    /// The session signer secret key (stored securely)
    #[serde(with = "serde_bytes")]
    pub session_signer_secret: Vec<u8>,

    /// Session token PDA on-chain
    pub session_token_pda: Option<Pubkey>,

    /// Session expiry timestamp (milliseconds)
    pub expires_at: i64,

    /// Optional game ID to join
    pub game_id: Option<String>,

    /// Role: 'host' or 'joiner'
    pub role: GameRole,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GameRole {
    Host,
    Joiner,
}

impl std::fmt::Display for GameRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GameRole::Host => write!(f, "host"),
            GameRole::Joiner => write!(f, "joiner"),
        }
    }
}

/// Error type for session operations
#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("Session not found")]
    NotFound,

    #[error("Session expired")]
    Expired,

    #[error("Invalid session data: {0}")]
    InvalidData(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

impl GameSession {
    /// Load session from environment or file
    pub fn load() -> Result<Self, SessionError> {
        // Try to load from environment variable first
        if let Ok(data) = std::env::var("XFCHESS_SESSION_DATA") {
            return Self::from_json(&data);
        }

        // Try to load from session file
        if let Ok(path) = std::env::var("XFCHESS_SESSION_FILE") {
            let data = std::fs::read_to_string(&path)?;
            return Self::from_json(&data);
        }

        // Check for session file in temp directory
        if let Some(session) = Self::find_temp_session()? {
            return Ok(session);
        }

        Err(SessionError::NotFound)
    }

    /// Parse session from JSON string
    pub fn from_json(json: &str) -> Result<Self, SessionError> {
        let session: GameSession = serde_json::from_str(json)?;

        // Validate session hasn't expired
        let now = chrono::Utc::now().timestamp_millis();
        if session.expires_at < now {
            return Err(SessionError::Expired);
        }

        Ok(session)
    }

    /// Find session file in temp directory
    fn find_temp_session() -> Result<Option<Self>, SessionError> {
        let temp_dir = std::env::temp_dir();

        // Look for xfchess_session_*.json files
        for entry in std::fs::read_dir(&temp_dir)? {
            let entry = entry?;
            let filename = entry.file_name();
            let filename_str = filename.to_string_lossy();

            if filename_str.starts_with("xfchess_session_") && filename_str.ends_with(".json") {
                let data = std::fs::read_to_string(entry.path())?;
                return Self::from_json(&data).map(Some);
            }
        }

        Ok(None)
    }

    /// Get the session keypair for signing transactions
    pub fn keypair(&self) -> Result<Keypair, SessionError> {
        let secret_key: [u8; 64] = self
            .session_signer_secret
            .as_slice()
            .try_into()
            .map_err(|_| SessionError::InvalidData("Invalid secret key length".to_string()))?;

        Keypair::try_from(&secret_key[..])
            .map_err(|e| SessionError::InvalidData(format!("Invalid keypair: {}", e)))
    }

    /// Check if session is still valid
    pub fn is_valid(&self) -> bool {
        let now = chrono::Utc::now().timestamp_millis();
        self.expires_at > now
    }

    /// Time remaining in seconds
    pub fn time_remaining(&self) -> i64 {
        let now = chrono::Utc::now().timestamp_millis();
        (self.expires_at - now) / 1000
    }
}

/// Resource to track session state in the game
#[derive(Debug, Default, Resource)]
pub struct SessionState {
    /// Current active session
    pub session: Option<GameSession>,

    /// Whether we're waiting for wallet signature (for joiners)
    pub awaiting_wallet_signature: bool,

    /// Last error message
    pub last_error: Option<String>,
}

/// Plugin to handle session initialization
pub struct SessionPlugin;

impl Plugin for SessionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SessionState>()
            .add_systems(Startup, initialize_session);
    }
}

/// Initialize session at startup
fn initialize_session(mut session_state: ResMut<SessionState>) {
    match GameSession::load() {
        Ok(session) => {
            info!(
                "Loaded game session for wallet: {}, role: {}",
                session.wallet_pubkey, session.role
            );
            info!("Session expires in {} seconds", session.time_remaining());

            session_state.session = Some(session);
            session_state.awaiting_wallet_signature = false;
        }
        Err(SessionError::NotFound) => {
            info!("No session found. Running in standalone mode.");
            session_state.session = None;
        }
        Err(SessionError::Expired) => {
            warn!("Session expired. Please create a new session from the web app.");
            session_state.last_error = Some("Session expired".to_string());
        }
        Err(e) => {
            error!("Failed to load session: {}", e);
            session_state.last_error = Some(e.to_string());
        }
    }
}

/// System to check session validity periodically
pub fn check_session_validity(mut session_state: ResMut<SessionState>) {
    if let Some(ref session) = session_state.session {
        if !session.is_valid() {
            warn!("Session has expired");
            session_state.session = None;
            session_state.last_error = Some("Session expired".to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_json_roundtrip() {
        let keypair = Keypair::new();
        let session = GameSession {
            wallet_pubkey: Pubkey::new_unique(),
            session_signer: keypair.pubkey(),
            session_signer_secret: keypair.to_bytes().to_vec(),
            session_token_pda: Some(Pubkey::new_unique()),
            expires_at: chrono::Utc::now().timestamp_millis() + 3600000,
            game_id: Some("test-game-123".to_string()),
            role: GameRole::Host,
        };

        let json = serde_json::to_string(&session).unwrap();
        let parsed = GameSession::from_json(&json).unwrap();

        assert_eq!(session.wallet_pubkey, parsed.wallet_pubkey);
        assert_eq!(session.session_signer, parsed.session_signer);
        assert_eq!(session.role, parsed.role);
    }

    #[test]
    fn test_session_expired() {
        let keypair = Keypair::new();
        let session = GameSession {
            wallet_pubkey: Pubkey::new_unique(),
            session_signer: keypair.pubkey(),
            session_signer_secret: keypair.to_bytes().to_vec(),
            session_token_pda: None,
            expires_at: chrono::Utc::now().timestamp_millis() - 1000,
            game_id: None,
            role: GameRole::Joiner,
        };

        assert!(!session.is_valid());
    }
}
