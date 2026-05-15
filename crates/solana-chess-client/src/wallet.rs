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
        let keypair = Keypair::from_bytes(&bytes)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keypair_wallet_generate_new() {
        let wallet = KeypairWallet::generate_new();
        assert!(!wallet.pubkey().to_string().is_empty());
    }

    #[test]
    fn keypair_wallet_sign_message() {
        let wallet = KeypairWallet::generate_new();
        let msg = b"hello chess";
        let sig = wallet.sign_message(msg);
        assert!(sig.verify(wallet.pubkey().as_ref(), msg));
    }

    #[test]
    fn keypair_wallet_keypair_returns_ref() {
        let wallet = KeypairWallet::generate_new();
        let kp = wallet.keypair();
        assert_eq!(kp.pubkey(), wallet.pubkey());
    }

    #[test]
    fn constant_seeds_match() {
        assert_eq!(super::super::GAME_SEED, b"game");
        assert_eq!(super::super::MOVE_LOG_SEED, b"move_log");
        assert_eq!(super::super::PROFILE_SEED, b"profile");
    }
}
