//! Session key manager for delegated transaction signing.
//!
//! This module handles:
//! - Generating ephemeral keypairs for session-based delegation
//! - Encrypting and storing session keys locally
//! - Authorizing session keys on-chain via CreateSession instruction
//! - Revoking session keys on logout
//! - Using session keys for silent transaction signing

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose, Engine as _};
use chrono::{DateTime, Utc};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use solana_sdk::{
    pubkey::Pubkey,
    signature::Signature,
    signature::{Keypair, Signer},
};
use std::path::PathBuf;
use std::sync::Arc;

/// Session key data stored encrypted on disk
#[derive(Serialize, Deserialize, Clone)]
pub struct SessionKeyData {
    /// The session key's public key
    pub session_pubkey: String,
    /// The session key's private key (encrypted)
    pub session_private_key: String,
    /// The main wallet's public key (owner)
    pub wallet_pubkey: String,
    /// When the session expires (Unix timestamp)
    pub expires_at: i64,
    /// When the session was created
    pub created_at: DateTime<Utc>,
}

/// Session key manager for handling ephemeral keypairs
pub struct SessionKeyManager {
    /// The ephemeral session keypair
    keypair: Arc<Keypair>,
    /// Encryption key for local storage (derived from wallet)
    encryption_key: Vec<u8>,
    /// Local data directory for storage
    data_dir: PathBuf,
}

impl SessionKeyManager {
    fn storage_dir() -> PathBuf {
        ProjectDirs::from("com", "xfchess", "XFChess")
            .map(|dirs| dirs.data_local_dir().to_path_buf())
            .unwrap_or_else(|| std::env::temp_dir().join("XFChess"))
    }

    /// Creates a new session key manager with a new ephemeral keypair.
    ///
    /// # Arguments
    /// * `wallet_pubkey` - The main wallet's public key (used for encryption key derivation)
    ///
    /// # Returns
    /// A new SessionKeyManager instance
    pub fn new(wallet_pubkey: &Pubkey) -> Self {
        let keypair = Arc::new(Keypair::new());
        let encryption_key = Self::derive_encryption_key(wallet_pubkey);
        let data_dir = Self::storage_dir();

        Self {
            keypair,
            encryption_key,
            data_dir,
        }
    }

    /// Derives encryption key from wallet pubkey using SHA-256
    fn derive_encryption_key(wallet_pubkey: &Pubkey) -> Vec<u8> {
        use sha2::{Digest, Sha256};
        let hash = Sha256::digest(wallet_pubkey.as_ref());
        hash.to_vec()
    }

    /// Returns the session key's public key
    pub fn pubkey(&self) -> Pubkey {
        self.keypair.pubkey()
    }

    /// Returns the session key's signing capability
    pub fn signer(&self) -> Arc<Keypair> {
        Arc::clone(&self.keypair)
    }

    /// Saves the session key data encrypted to local storage.
    ///
    /// # Arguments
    /// * `wallet_pubkey` - The main wallet's public key
    /// * `duration_hours` - Session duration in hours
    pub fn save_session(&self, wallet_pubkey: &Pubkey, duration_hours: i64) -> Result<(), String> {
        let session_data = SessionKeyData {
            session_pubkey: self.keypair.pubkey().to_string(),
            session_private_key: bs58::encode(self.keypair.to_bytes()).into_string(),
            wallet_pubkey: wallet_pubkey.to_string(),
            expires_at: Utc::now().timestamp() + (duration_hours * 3600),
            created_at: Utc::now(),
        };

        // Serialize to JSON
        let json = serde_json::to_string(&session_data)
            .map_err(|e| format!("Failed to serialize session data: {}", e))?;

        // Encrypt
        let cipher = Aes256Gcm::new_from_slice(&self.encryption_key)
            .map_err(|e| format!("Failed to create cipher: {}", e))?;

        #[allow(deprecated)]
        let nonce = Nonce::from_slice(b"xfchess sess"); // 12 bytes for AES-256-GCM
        let encrypted = cipher
            .encrypt(nonce, json.as_bytes())
            .map_err(|e| format!("Failed to encrypt: {}", e))?;

        // Encode to base64
        let encoded = general_purpose::STANDARD.encode(encrypted);

        // Save to file
        std::fs::create_dir_all(&self.data_dir)
            .map_err(|e| format!("Failed to create data directory: {}", e))?;

        let session_file = self.data_dir.join("session_key.enc");
        std::fs::write(&session_file, encoded)
            .map_err(|e| format!("Failed to write session file: {}", e))?;

        Ok(())
    }

    /// Loads and decrypts session key data from local storage.
    ///
    /// # Arguments
    /// * `wallet_pubkey` - The main wallet's public key (for decryption)
    ///
    /// # Returns
    /// The reconstructed SessionKeyManager if found and valid
    pub fn load_session(wallet_pubkey: &Pubkey) -> Result<Self, String> {
        let data_dir = Self::storage_dir();
        let session_file = data_dir.join("session_key.enc");

        if !session_file.exists() {
            return Err("No session file found".to_string());
        }

        // Read encrypted data
        let encoded = std::fs::read_to_string(&session_file)
            .map_err(|e| format!("Failed to read session file: {}", e))?;

        // Decode from base64
        let encrypted = general_purpose::STANDARD
            .decode(encoded)
            .map_err(|e| format!("Failed to decode base64: {}", e))?;

        // Decrypt
        let encryption_key = Self::derive_encryption_key(wallet_pubkey);
        let cipher = Aes256Gcm::new_from_slice(&encryption_key)
            .map_err(|e| format!("Failed to create cipher: {}", e))?;

        #[allow(deprecated)]
        let nonce = Nonce::from_slice(b"xfchess sess"); // 12 bytes for AES-256-GCM
        let decrypted = cipher
            .decrypt(nonce, encrypted.as_ref())
            .map_err(|e| format!("Failed to decrypt: {}", e))?;

        // Deserialize
        let session_data: SessionKeyData = serde_json::from_slice(&decrypted)
            .map_err(|e| format!("Failed to deserialize: {}", e))?;

        // Check if session belongs to this wallet
        if session_data.wallet_pubkey != wallet_pubkey.to_string() {
            return Err("Session belongs to different wallet".to_string());
        }

        // Check if session is expired
        let now = Utc::now().timestamp();
        if session_data.expires_at < now {
            return Err("Session expired".to_string());
        }

        // Reconstruct keypair from private key
        let private_key_bytes = bs58::decode(&session_data.session_private_key)
            .into_vec()
            .map_err(|e| format!("Failed to decode private key: {}", e))?;

        let keypair = Keypair::try_from(private_key_bytes.as_slice())
            .map_err(|e| format!("Failed to reconstruct keypair: {}", e))?;

        Ok(Self {
            keypair: Arc::new(keypair),
            encryption_key,
            data_dir,
        })
    }

    /// Deletes the session key file from local storage.
    pub fn delete_session() -> Result<(), String> {
        let data_dir = Self::storage_dir();
        let session_file = data_dir.join("session_key.enc");

        if session_file.exists() {
            std::fs::remove_file(&session_file)
                .map_err(|e| format!("Failed to delete session file: {}", e))?;
        }

        Ok(())
    }

    /// Signs a message with the session key.
    ///
    /// # Arguments
    /// * `message` - The message bytes to sign
    ///
    /// # Returns
    /// The signature
    pub fn sign(&self, message: &[u8]) -> Signature {
        self.keypair.sign_message(message)
    }

    /// Returns the Unix timestamp at which this session expires, if it can be read from disk.
    pub fn expires_at(wallet_pubkey: &Pubkey) -> Option<i64> {
        let data_dir = Self::storage_dir();
        let session_file = data_dir.join("session_key.enc");
        let encryption_key = Self::derive_encryption_key(wallet_pubkey);
        let encrypted = std::fs::read(&session_file).ok()?;
        let cipher = Aes256Gcm::new_from_slice(&encryption_key).ok()?;
        #[allow(deprecated)]
        let nonce = Nonce::from_slice(b"xfchess sess");
        let decrypted = cipher.decrypt(nonce, encrypted.as_ref()).ok()?;
        let data: SessionKeyData = serde_json::from_slice(&decrypted).ok()?;
        Some(data.expires_at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encryption_key_derivation() {
        let pubkey = Pubkey::new_unique();
        let key1 = SessionKeyManager::derive_encryption_key(&pubkey);
        let key2 = SessionKeyManager::derive_encryption_key(&pubkey);
        assert_eq!(key1, key2);
    }

    #[test]
    fn test_session_key_generation() {
        let wallet_pubkey = Pubkey::new_unique();
        let manager = SessionKeyManager::new(&wallet_pubkey);

        let session_pubkey = manager.pubkey();
        assert_ne!(session_pubkey, wallet_pubkey);

        let message = b"test message";
        let signature = manager.sign(message);
        assert!(signature.verify(&session_pubkey.as_ref(), message));
    }
}
