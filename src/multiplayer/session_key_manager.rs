#![cfg(feature = "solana")]
use bevy::prelude::*;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};
use std::path::PathBuf;
use tokio::fs;

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
        Self {
            game_id,
            session_keypair: None,
        }
    }

    pub async fn load_or_create_session_keypair(
        &mut self,
    ) -> Result<Keypair, Box<dyn std::error::Error>> {
        if let Some(keypair) = &self.session_keypair {
            return Ok(keypair.insecure_clone());
        }

        if let Ok(keypair) = self.load_keypair_from_disk().await {
            self.session_keypair = Some(keypair.insecure_clone());
            return Ok(keypair);
        }

        let keypair = Keypair::new();
        self.save_keypair_to_disk(&keypair).await?;
        self.session_keypair = Some(keypair.insecure_clone());
        Ok(keypair)
    }

    async fn save_keypair_to_disk(
        &self,
        keypair: &Keypair,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let keypair_bytes = keypair.to_bytes();
        let key_path = self.get_key_path()?;
        if let Some(parent) = key_path.parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::write(&key_path, keypair_bytes).await?;
        Ok(())
    }

    async fn load_keypair_from_disk(&self) -> Result<Keypair, Box<dyn std::error::Error>> {
        let key_path = self.get_key_path()?;
        let keypair_bytes = fs::read(&key_path).await?;
        if keypair_bytes.len() != 64 {
            return Err("Invalid keypair file length".into());
        }
        let mut array = [0u8; 64];
        array.copy_from_slice(&keypair_bytes);
        Ok(Keypair::try_from(&array[..])?)
    }

    fn get_key_path(&self) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let path = dirs::data_dir()
            .ok_or("Could not determine data directory")?
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
    }

    pub fn set_game_id(&mut self, game_id: u64) {
        if self.game_id != game_id {
            self.game_id = game_id;
            self.session_keypair = None;
        }
    }
}
