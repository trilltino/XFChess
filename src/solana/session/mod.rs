use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};
use std::path::Path;
use std::time::SystemTime;

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

    #[error("Invalid session signer: {0}")]
    InvalidSigner(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

impl GameSession {
    pub fn load() -> Result<Self, SessionError> {
        if let Ok(data) = std::env::var("XFCHESS_SESSION_DATA") {
            return Self::from_json(&data);
        }

        if let Ok(path) = std::env::var("XFCHESS_SESSION_FILE") {
            let data = std::fs::read_to_string(&path)?;
            return Self::from_json(&data);
        }

        if let Some(session) = Self::find_temp_session()? {
            return Ok(session);
        }

        Err(SessionError::NotFound)
    }

    pub fn from_json(json: &str) -> Result<Self, SessionError> {
        let session: GameSession = serde_json::from_str(json)?;

        let now = chrono::Utc::now().timestamp_millis();
        if session.expires_at < now {
            return Err(SessionError::Expired);
        }
        session.validate_signer()?;

        Ok(session)
    }

    fn find_temp_session() -> Result<Option<Self>, SessionError> {
        let temp_dir = std::env::temp_dir();
        Self::find_temp_session_in(&temp_dir)
    }

    fn find_temp_session_in(temp_dir: &Path) -> Result<Option<Self>, SessionError> {
        let mut best: Option<(SystemTime, Self)> = None;
        for entry in std::fs::read_dir(temp_dir)? {
            let entry = entry?;
            let filename = entry.file_name();
            let filename_str = filename.to_string_lossy();
            if filename_str.starts_with("xfchess_session_") && filename_str.ends_with(".json") {
                let path = entry.path();
                let modified = entry
                    .metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(SystemTime::UNIX_EPOCH);

                match std::fs::read_to_string(&path)
                    .map_err(SessionError::from)
                    .and_then(|data| Self::from_json(&data))
                {
                    Ok(session) => {
                        let replace = best
                            .as_ref()
                            .map(|(best_modified, _)| modified > *best_modified)
                            .unwrap_or(true);
                        if replace {
                            best = Some((modified, session));
                        }
                    }
                    Err(SessionError::Expired) => {
                        warn!("Skipping expired temp session: {}", path.display());
                    }
                    Err(e) => {
                        warn!("Skipping invalid temp session {}: {}", path.display(), e);
                    }
                }
            }
        }

        Ok(best.map(|(_, session)| session))
    }

    pub fn time_remaining(&self) -> i64 {
        let now = chrono::Utc::now().timestamp_millis();
        (self.expires_at - now) / 1000
    }

    pub fn is_valid(&self) -> bool {
        self.expires_at >= chrono::Utc::now().timestamp_millis()
    }

    fn validate_signer(&self) -> Result<(), SessionError> {
        let keypair = Keypair::try_from(self.session_signer_secret.as_slice())
            .map_err(|e| SessionError::InvalidSigner(e.to_string()))?;
        if keypair.pubkey() != self.session_signer {
            return Err(SessionError::InvalidSigner(format!(
                "secret key derives {}, expected {}",
                keypair.pubkey(),
                self.session_signer
            )));
        }
        Ok(())
    }

    pub fn short_wallet(&self) -> String {
        short_pubkey(&self.wallet_pubkey)
    }

    pub fn short_session_signer(&self) -> String {
        short_pubkey(&self.session_signer)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionHealth {
    Standalone,
    Connected,
    Expired,
    Invalid,
    LoadFailed,
}

impl Default for SessionHealth {
    fn default() -> Self {
        Self::Standalone
    }
}

impl SessionHealth {
    pub fn label(self) -> &'static str {
        match self {
            SessionHealth::Standalone => "Standalone mode",
            SessionHealth::Connected => "Imported session connected",
            SessionHealth::Expired => "Session expired",
            SessionHealth::Invalid => "Session invalid",
            SessionHealth::LoadFailed => "Session load failed",
        }
    }

    pub fn session_kind(self) -> &'static str {
        match self {
            SessionHealth::Standalone => "standalone",
            SessionHealth::Connected => "imported",
            SessionHealth::Expired => "expired",
            SessionHealth::Invalid => "invalid",
            SessionHealth::LoadFailed => "load_failed",
        }
    }
}

#[derive(Debug, Resource)]
pub struct SessionState {
    pub session: Option<GameSession>,
    pub awaiting_wallet_signature: bool,
    pub last_error: Option<String>,
    pub health: SessionHealth,
    pub recovery_message: Option<String>,
}

impl Default for SessionState {
    fn default() -> Self {
        Self {
            session: None,
            awaiting_wallet_signature: false,
            last_error: None,
            health: SessionHealth::Standalone,
            recovery_message: None,
        }
    }
}

impl SessionState {
    pub fn status_label(&self) -> &'static str {
        self.health.label()
    }

    pub fn session_kind(&self) -> &'static str {
        self.health.session_kind()
    }

    pub fn wallet_pubkey(&self) -> Option<Pubkey> {
        self.session.as_ref().map(|s| s.wallet_pubkey)
    }

    pub fn game_id(&self) -> Option<&str> {
        self.session.as_ref().and_then(|s| s.game_id.as_deref())
    }

    pub fn role(&self) -> Option<GameRole> {
        self.session.as_ref().map(|s| s.role)
    }

    pub fn time_remaining_secs(&self) -> Option<i64> {
        self.session.as_ref().map(|s| s.time_remaining().max(0))
    }
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
            session_state.health = SessionHealth::Connected;
            session_state.last_error = None;
            session_state.recovery_message = None;
            crate::multiplayer::network::vps::emit_client_event(
                crate::multiplayer::network::vps::ClientEvent::new("solana_session_loaded")
                    .session_kind("imported"),
            );
        }
        Err(SessionError::NotFound) => {
            info!("No session found. Running in standalone mode.");
            session_state.session = None;
            session_state.health = SessionHealth::Standalone;
            session_state.recovery_message = None;
            crate::multiplayer::network::vps::emit_client_event(
                crate::multiplayer::network::vps::ClientEvent::new("solana_session_standalone")
                    .session_kind("standalone"),
            );
        }
        Err(SessionError::Expired) => {
            let msg =
                "Your Solana session expired. Reconnect your wallet from the launcher/web app.";
            warn!("{msg}");
            session_state.health = SessionHealth::Expired;
            session_state.last_error = Some("Session expired".to_string());
            session_state.recovery_message = Some(msg.to_string());
            crate::multiplayer::network::vps::emit_client_event(
                crate::multiplayer::network::vps::ClientEvent::new("solana_session_expired")
                    .session_kind("expired")
                    .reason("expired"),
            );
        }
        Err(SessionError::InvalidSigner(e)) => {
            let msg = format!("Session signer is invalid. Reconnect your wallet from the launcher/web app. ({e})");
            warn!("{msg}");
            session_state.health = SessionHealth::Invalid;
            session_state.last_error = Some("Invalid session signer".to_string());
            session_state.recovery_message = Some(msg);
            crate::multiplayer::network::vps::emit_client_event(
                crate::multiplayer::network::vps::ClientEvent::new("solana_session_invalid_signer")
                    .session_kind("invalid")
                    .reason(e),
            );
        }
        Err(e) => {
            error!("Failed to load session: {}", e);
            session_state.health = SessionHealth::LoadFailed;
            session_state.last_error = Some(e.to_string());
            session_state.recovery_message = Some(
                "Solana session could not be loaded. Reconnect your wallet from the launcher/web app."
                    .to_string(),
            );
            crate::multiplayer::network::vps::emit_client_event(
                crate::multiplayer::network::vps::ClientEvent::new("solana_session_load_failed")
                    .session_kind("load_failed")
                    .reason(e.to_string()),
            );
        }
    }
}

fn short_pubkey(pubkey: &Pubkey) -> String {
    let s = pubkey.to_string();
    format!("{}...{}", &s[..6], &s[s.len() - 4..])
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

    #[test]
    fn test_session_invalid_signer_secret() {
        let keypair = Keypair::new();
        let mut session = GameSession {
            wallet_pubkey: Pubkey::new_unique(),
            session_signer: keypair.pubkey(),
            session_signer_secret: vec![1, 2, 3],
            session_token_pda: None,
            expires_at: chrono::Utc::now().timestamp_millis() + 3600000,
            game_id: None,
            role: GameRole::Joiner,
        };

        let json = serde_json::to_string(&session).unwrap();
        assert!(matches!(
            GameSession::from_json(&json),
            Err(SessionError::InvalidSigner(_))
        ));

        let other = Keypair::new();
        session.session_signer_secret = other.to_bytes().to_vec();
        let json = serde_json::to_string(&session).unwrap();
        assert!(matches!(
            GameSession::from_json(&json),
            Err(SessionError::InvalidSigner(_))
        ));
    }

    #[test]
    fn test_find_temp_session_chooses_newest_valid() {
        let dir = unique_test_dir();
        std::fs::create_dir_all(&dir).unwrap();

        let old_keypair = Keypair::new();
        let old = GameSession {
            wallet_pubkey: Pubkey::new_unique(),
            session_signer: old_keypair.pubkey(),
            session_signer_secret: old_keypair.to_bytes().to_vec(),
            session_token_pda: None,
            expires_at: chrono::Utc::now().timestamp_millis() + 3600000,
            game_id: Some("old".to_string()),
            role: GameRole::Host,
        };
        std::fs::write(
            dir.join("xfchess_session_old.json"),
            serde_json::to_string(&old).unwrap(),
        )
        .unwrap();

        std::thread::sleep(std::time::Duration::from_millis(10));
        std::fs::write(dir.join("xfchess_session_bad.json"), "{").unwrap();

        std::thread::sleep(std::time::Duration::from_millis(10));
        let new_keypair = Keypair::new();
        let new = GameSession {
            wallet_pubkey: Pubkey::new_unique(),
            session_signer: new_keypair.pubkey(),
            session_signer_secret: new_keypair.to_bytes().to_vec(),
            session_token_pda: None,
            expires_at: chrono::Utc::now().timestamp_millis() + 3600000,
            game_id: Some("new".to_string()),
            role: GameRole::Joiner,
        };
        std::fs::write(
            dir.join("xfchess_session_new.json"),
            serde_json::to_string(&new).unwrap(),
        )
        .unwrap();

        let found = GameSession::find_temp_session_in(&dir).unwrap().unwrap();
        assert_eq!(found.game_id.as_deref(), Some("new"));

        let _ = std::fs::remove_dir_all(dir);
    }

    fn unique_test_dir() -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "xfchess_session_test_{}_{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ))
    }
}
