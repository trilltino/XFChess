use anyhow::Result;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};
use std::fs;
use std::path::PathBuf;

/// Provides a signing wallet for the Bevy client
pub trait Wallet {
    fn pubkey(&self) -> Pubkey;
    fn sign_message(&self, message: &[u8]) -> solana_sdk::signature::Signature;
    fn keypair(&self) -> &Keypair;
}

pub struct KeypairWallet {
    keypair: Keypair,
}

impl KeypairWallet {
    pub fn new(keypair: Keypair) -> Self {
        Self { keypair }
    }

    /// Loads a wallet from a standard Solana CLI JSON keypair file
    pub fn load_from_file(path: &PathBuf) -> Result<Self> {
        let data = fs::read_to_string(path)?;
        let bytes: Vec<u8> = serde_json::from_str(&data)?;
        let mut secret = [0u8; 32];
        let len = bytes.len().min(32);
        secret[..len].copy_from_slice(&bytes[..len]);
        let keypair = Keypair::new_from_array(secret);
        Ok(Self { keypair })
    }

    /// Generates a new random wallet (useful for 'Link' mode ephemeral profiles)
    pub fn generate_new() -> Self {
        Self {
            keypair: Keypair::new(),
        }
    }
}

impl Wallet for KeypairWallet {
    fn pubkey(&self) -> Pubkey {
        self.keypair.pubkey()
    }

    fn sign_message(&self, message: &[u8]) -> solana_sdk::signature::Signature {
        self.keypair.sign_message(message)
    }

    fn keypair(&self) -> &Keypair {
        &self.keypair
    }
}
