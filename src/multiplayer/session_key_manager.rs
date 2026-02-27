#![cfg(feature = "solana")]
use bevy::prelude::*;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use thiserror::Error;

/// Errors that can occur during session key management
#[derive(Error, Debug)]
pub enum SessionKeyError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("Invalid keypair data: {0}")]
    InvalidKeypair(String),

    #[error("Data directory not found")]
    DataDirNotFound,

    #[error("Serialization error: {0}")]
    Serialization(String),
}

#[derive(Resource)]
pub struct SessionKeyManager {
    game_id: u64,
    session_keypair: Option<Keypair>,
}

impl Default for SessionKeyManager {
    fn default() -> Self {
        Self {
            game_id: 0,
            session_keypair: None,
        }
    }
}

impl SessionKeyManager {
    pub fn new(game_id: u64) -> Self {
        let mut manager = Self {
            game_id,
            session_keypair: None,
        };
        // Try to load existing keypair on creation
        if let Ok(keypair) = manager.load_keypair_from_disk() {
            manager.session_keypair = Some(keypair);
        }
        manager
    }

    /// Load or create a session keypair synchronously
    /// This is the primary method for use in Bevy systems
    pub fn load_or_create_keypair(&mut self) -> Result<Keypair, SessionKeyError> {
        if let Some(keypair) = &self.session_keypair {
            return Ok(keypair.insecure_clone());
        }

        // Try to load existing keypair
        if let Ok(keypair) = self.load_keypair_from_disk() {
            self.session_keypair = Some(keypair.insecure_clone());
            return Ok(keypair);
        }

        // Create new keypair if none exists
        let keypair = Keypair::new();
        self.save_keypair_to_disk(&keypair)?;
        self.session_keypair = Some(keypair.insecure_clone());
        Ok(keypair)
    }

    /// Synchronous version for use in async contexts
    pub fn load_or_create_session_keypair_sync(
        &mut self,
    ) -> Result<Keypair, Box<dyn std::error::Error>> {
        self.load_or_create_keypair()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }

    fn save_keypair_to_disk(&self, keypair: &Keypair) -> Result<(), SessionKeyError> {
        let keypair_bytes = keypair.to_bytes();
        let key_path = self.get_key_path()?;

        // Create parent directories synchronously
        if let Some(parent) = key_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Write file synchronously
        let mut file = fs::File::create(&key_path)?;
        file.write_all(&keypair_bytes)?;
        file.sync_all()?; // Ensure data is written to disk

        // Set restrictive permissions (0o600 on Unix, ignored on Windows)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = fs::metadata(&key_path)?.permissions();
            permissions.set_mode(0o600);
            fs::set_permissions(&key_path, permissions)?;
        }

        info!("Session keypair saved to {:?}", key_path);
        Ok(())
    }

    fn load_keypair_from_disk(&self) -> Result<Keypair, SessionKeyError> {
        let key_path = self.get_key_path()?;

        let mut file = fs::File::open(&key_path)?;
        let mut keypair_bytes = Vec::new();
        file.read_to_end(&mut keypair_bytes)?;

        if keypair_bytes.len() != 64 {
            return Err(SessionKeyError::InvalidKeypair(format!(
                "Expected 64 bytes, got {}",
                keypair_bytes.len()
            )));
        }

        let mut array = [0u8; 64];
        array.copy_from_slice(&keypair_bytes);

        Keypair::try_from(&array[..]).map_err(|e| SessionKeyError::InvalidKeypair(e.to_string()))
    }

    fn get_key_path(&self) -> Result<PathBuf, SessionKeyError> {
        let path = dirs::data_dir()
            .ok_or(SessionKeyError::DataDirNotFound)?
            .join("xfchess")
            .join("session_keys")
            .join(format!("game_{}.key", self.game_id));
        Ok(path)
    }

    pub fn get_session_pubkey(&self) -> Option<Pubkey> {
        self.session_keypair.as_ref().map(|kp| kp.pubkey())
    }

    pub fn get_session_keypair(&self) -> Option<&Keypair> {
        self.session_keypair.as_ref()
    }

    pub fn clear_session_keypair(&mut self) {
        self.session_keypair = None;
        // Also remove from disk
        if let Ok(key_path) = self.get_key_path() {
            if let Err(e) = fs::remove_file(&key_path) {
                warn!("Failed to remove session key file: {}", e);
            }
        }
    }

    pub fn set_game_id(&mut self, game_id: u64) {
        if self.game_id != game_id {
            self.game_id = game_id;
            self.session_keypair = None;
            // Try to load keypair for new game_id
            if let Ok(keypair) = self.load_keypair_from_disk() {
                self.session_keypair = Some(keypair);
            }
        }
    }

    /// Get the current game ID
    pub fn game_id(&self) -> u64 {
        self.game_id
    }

    /// Check if a session keypair is loaded
    pub fn has_keypair(&self) -> bool {
        self.session_keypair.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keypair_roundtrip() {
        let mut manager = SessionKeyManager::new(999999); // Use unlikely game ID

        // Clear any existing keypair
        manager.clear_session_keypair();
        manager.session_keypair = None;

        // Create a new keypair
        let keypair = manager
            .load_or_create_keypair()
            .expect("Failed to create keypair");
        let pubkey = keypair.pubkey();

        // Create a new manager with the same game ID
        let mut manager2 = SessionKeyManager::new(999999);
        let loaded_keypair = manager2
            .load_or_create_keypair()
            .expect("Failed to load keypair");

        // The loaded keypair should have the same public key
        assert_eq!(keypair.pubkey(), loaded_keypair.pubkey());
        assert_eq!(pubkey, loaded_keypair.pubkey());

        // Cleanup
        manager.clear_session_keypair();
    }

    #[test]
    fn test_different_game_ids() {
        let manager1 = SessionKeyManager::new(111111);
        let manager2 = SessionKeyManager::new(222222);

        // Different game IDs should result in different key paths
        let path1 = manager1.get_key_path().unwrap();
        let path2 = manager2.get_key_path().unwrap();

        assert_ne!(path1, path2);
    }
}
