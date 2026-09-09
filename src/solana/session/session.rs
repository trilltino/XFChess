use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;

#[derive(Debug, Clone, Serialize, Deserialize, Resource)]
pub struct GameSession {
    pub wallet_pubkey: Pubkey,

    pub session_signer: Pubkey,

    #[serde(with = "serde_bytes")]
    pub session_signer_secret: Vec<u8>,

    pub session_token_pda: Option<Pubkey>,

    pub expires_at: i64,

    pub game_id: Option<String>,

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

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("Session not found")]
    NotFound,

    #[error("Session expired")]
    Expired,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

impl GameSession {
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

    pub fn from_json(json: &str) -> Result<Self, SessionError> {
        let session: GameSession = serde_json::from_str(json)?;

        // Validate session hasn't expired
        let now = chrono::Utc::now().timestamp_millis();
        if session.expires_at < now {
            return Err(SessionError::Expired);
        }

        Ok(session)
    }

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

    pub fn time_remaining(&self) -> i64 {
        let now = chrono::Utc::now().timestamp_millis();
        (self.expires_at - now) / 1000
    }

    pub fn is_valid(&self) -> bool {
        self.expires_at >= chrono::Utc::now().timestamp_millis()
    }
}

#[derive(Debug, Default, Resource)]
pub struct SessionState {
    pub session: Option<GameSession>,

    pub awaiting_wallet_signature: bool,

    pub last_error: Option<String>,
}

pub struct SessionPlugin;

impl Plugin for SessionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SessionState>()
            .add_systems(Startup, initialize_session);
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::signature::{Keypair, Signer};

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
