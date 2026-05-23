//! Global persistent session key management for client-side use.
//!
//! One session keypair per wallet, persisted on disk, good for 30 days / 200
//! games. After `authorize_global_session` succeeds the VPS can co-sign every
//! `global_create_game` / `global_join_game` without another wallet popup.
//!
//! Storage: `<data-dir>/global_session_key.enc`  (AES-256-GCM, same pattern as
//! [`SessionKeyManager`](super::session_key_manager::SessionKeyManager)).

use solana_sdk::{
    instruction::AccountMeta,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
#[allow(deprecated)]
use solana_sdk::system_program;
use std::path::PathBuf;
use std::sync::Arc;
use directories::ProjectDirs;
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{Engine as _, engine::general_purpose};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// PDA seed prefix matching [`GlobalSessionDelegation::SEED`] on-chain.
const SEED: &[u8] = b"global_session";

// ── PDA helper ────────────────────────────────────────────────────────────────

/// Derive the `GlobalSessionDelegation` PDA for `player`.
pub fn find_global_session_pda(program_id: &Pubkey, player: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[SEED, player.as_ref()], program_id)
}

// ── Disk-persisted global session keypair ─────────────────────────────────────

/// Encrypted session key record stored on disk.
#[derive(Serialize, Deserialize, Clone)]
pub struct GlobalSessionKeyData {
    pub session_pubkey: String,
    pub session_private_key: String,
    pub wallet_pubkey: String,
    pub expires_at: i64,
    pub created_at: DateTime<Utc>,
}

/// Manages a single global session keypair for a wallet.
pub struct GlobalSessionKeyManager {
    keypair: Arc<Keypair>,
    encryption_key: Vec<u8>,
    data_dir: PathBuf,
}

impl GlobalSessionKeyManager {
    fn storage_dir() -> PathBuf {
        directories::ProjectDirs::from("com", "xfchess", "XFChess")
            .map(|dirs| dirs.data_local_dir().to_path_buf())
            .unwrap_or_else(|| std::env::temp_dir().join("XFChess"))
    }

    fn derive_key(wallet: &Pubkey) -> Vec<u8> {
        use sha2::{Digest, Sha256};
        Sha256::digest(wallet.as_ref()).to_vec()
    }

    /// Create a fresh session keypair for `wallet`.
    pub fn new(wallet: &Pubkey) -> Self {
        Self {
            keypair: Arc::new(Keypair::new()),
            encryption_key: Self::derive_key(wallet),
            data_dir: Self::storage_dir(),
        }
    }

    /// Load an existing persisted session, or create one if absent/expired.
    pub fn load_or_create(wallet: &Pubkey) -> Self {
        Self::load(wallet).unwrap_or_else(|_| Self::new(wallet))
    }

    pub fn pubkey(&self) -> Pubkey {
        self.keypair.pubkey()
    }

    pub fn signer(&self) -> Arc<Keypair> {
        Arc::clone(&self.keypair)
    }

    /// Persist the session keypair encrypted to `global_session_key.enc`.
    pub fn save(&self, wallet: &Pubkey, duration_days: i64) -> Result<(), String> {
        let data = GlobalSessionKeyData {
            session_pubkey: self.keypair.pubkey().to_string(),
            session_private_key: bs58::encode(self.keypair.to_bytes()).into_string(),
            wallet_pubkey: wallet.to_string(),
            expires_at: Utc::now().timestamp() + duration_days * 86_400,
            created_at: Utc::now(),
        };
        let json = serde_json::to_string(&data)
            .map_err(|e| format!("serialize: {e}"))?;
        let cipher = Aes256Gcm::new_from_slice(&self.encryption_key)
            .map_err(|e| format!("cipher: {e}"))?;
        #[allow(deprecated)]
        let nonce = Nonce::from_slice(b"xfchess glob"); // 12 bytes
        let encrypted = cipher.encrypt(nonce, json.as_bytes())
            .map_err(|e| format!("encrypt: {e}"))?;
        let encoded = general_purpose::STANDARD.encode(encrypted);
        std::fs::create_dir_all(&self.data_dir)
            .map_err(|e| format!("mkdir: {e}"))?;
        std::fs::write(self.data_dir.join("global_session_key.enc"), encoded)
            .map_err(|e| format!("write: {e}"))
    }

    /// Load from disk; returns `Err` if the file is missing, expired, or
    /// belongs to a different wallet.
    pub fn load(wallet: &Pubkey) -> Result<Self, String> {
        let data_dir = Self::storage_dir();
        let path = data_dir.join("global_session_key.enc");
        if !path.exists() {
            return Err("no file".into());
        }
        let encoded = std::fs::read_to_string(&path)
            .map_err(|e| format!("read: {e}"))?;
        let encrypted = general_purpose::STANDARD.decode(encoded)
            .map_err(|e| format!("base64: {e}"))?;
        let key = Self::derive_key(wallet);
        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|e| format!("cipher: {e}"))?;
        #[allow(deprecated)]
        let nonce = Nonce::from_slice(b"xfchess glob");
        let decrypted = cipher.decrypt(nonce, encrypted.as_ref())
            .map_err(|e| format!("decrypt: {e}"))?;
        let data: GlobalSessionKeyData = serde_json::from_slice(&decrypted)
            .map_err(|e| format!("deserialize: {e}"))?;
        if data.wallet_pubkey != wallet.to_string() {
            return Err("wallet mismatch".into());
        }
        if data.expires_at < Utc::now().timestamp() {
            return Err("expired".into());
        }
        let bytes = bs58::decode(&data.session_private_key)
            .into_vec()
            .map_err(|e| format!("decode key: {e}"))?;
        let kp = Keypair::from_bytes(&bytes)
            .map_err(|e| format!("keypair: {e}"))?;
        Ok(Self {
            keypair: Arc::new(kp),
            encryption_key: key,
            data_dir,
        })
    }

    /// Remove the persisted session file.
    pub fn delete() -> Result<(), String> {
        let path = Self::storage_dir().join("global_session_key.enc");
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| format!("delete: {e}"))?;
        }
        Ok(())
    }
}

// ── Instruction builders ──────────────────────────────────────────────────────

/// Arguments matching [`AuthorizeGlobalSessionArgs`] on-chain.
#[derive(Debug, Clone)]
pub struct AuthorizeGlobalSessionArgs {
    pub session_key: Pubkey,
    pub duration_secs: Option<i64>,
    pub spending_limit: Option<u64>,
    pub max_wager: Option<u64>,
    pub games: Option<u16>,
    pub deposit_lamports: u64,
}

/// Build an `authorize_global_session` instruction.
///
/// sha256("global:authorize_global_session")[..8]
pub fn build_authorize_global_session_ix(
    program_id: &Pubkey,
    player: &Pubkey,
    session_pda: &Pubkey,
    args: AuthorizeGlobalSessionArgs,
) -> solana_sdk::instruction::Instruction {
    let discriminator: [u8; 8] = [0x15, 0xd3, 0x8a, 0x6c, 0xf2, 0x71, 0x4e, 0xb2];

    let mut data = Vec::with_capacity(128);
    data.extend_from_slice(&discriminator);
    // AuthorizeGlobalSessionArgs (Borsh layout)
    data.extend_from_slice(args.session_key.as_ref());
    push_option_i64(&mut data, args.duration_secs);
    push_option_u64(&mut data, args.spending_limit);
    push_option_u64(&mut data, args.max_wager);
    push_option_u16(&mut data, args.games);
    data.extend_from_slice(&args.deposit_lamports.to_le_bytes());

    let accounts = vec![
        AccountMeta::new(*session_pda, false),
        AccountMeta::new(*player, true),
        AccountMeta::new_readonly(system_program::id(), false),
    ];

    solana_sdk::instruction::Instruction { program_id: *program_id, accounts, data }
}

/// Build a `revoke_global_session` instruction.
pub fn build_revoke_global_session_ix(
    program_id: &Pubkey,
    player: &Pubkey,
    session_pda: &Pubkey,
) -> solana_sdk::instruction::Instruction {
    let discriminator: [u8; 8] = [0x3c, 0x8b, 0x4a, 0x11, 0xdd, 0x60, 0x92, 0x5f];

    let accounts = vec![
        AccountMeta::new(*session_pda, false),
        AccountMeta::new(*player, true),
    ];

    solana_sdk::instruction::Instruction {
        program_id: *program_id,
        accounts,
        data: discriminator.to_vec(),
    }
}

/// Build a `global_create_game` instruction.
pub fn build_global_create_game_ix(
    program_id: &Pubkey,
    session_pda: &Pubkey,
    session_signer: &Pubkey,
    player: &Pubkey,
    game_pda: &Pubkey,
    escrow_pda: &Pubkey,
    game_id: u64,
    wager_amount: u64,
    match_type: u8,
    country: &str,
    base_time_seconds: u64,
    increment_seconds: u16,
) -> solana_sdk::instruction::Instruction {
    let discriminator: [u8; 8] = [0x7f, 0x2c, 0x5d, 0xe8, 0xa1, 0x93, 0x4b, 0x06];

    let mut data = Vec::with_capacity(128);
    data.extend_from_slice(&discriminator);
    data.extend_from_slice(&game_id.to_le_bytes());
    data.extend_from_slice(&wager_amount.to_le_bytes());
    data.push(match_type);
    // String (Borsh: u32 len + bytes)
    let country_bytes = country.as_bytes();
    data.extend_from_slice(&(country_bytes.len() as u32).to_le_bytes());
    data.extend_from_slice(country_bytes);
    data.extend_from_slice(&base_time_seconds.to_le_bytes());
    data.extend_from_slice(&increment_seconds.to_le_bytes());

    let accounts = vec![
        AccountMeta::new(*session_pda, false),
        AccountMeta::new_readonly(*session_signer, true),
        AccountMeta::new_readonly(*player, false),
        AccountMeta::new(*game_pda, false),
        AccountMeta::new(*escrow_pda, false),
        AccountMeta::new_readonly(system_program::id(), false),
    ];

    solana_sdk::instruction::Instruction { program_id: *program_id, accounts, data }
}

/// Build a `global_join_game` instruction.
pub fn build_global_join_game_ix(
    program_id: &Pubkey,
    session_pda: &Pubkey,
    session_signer: &Pubkey,
    player: &Pubkey,
    game_pda: &Pubkey,
    player_profile_pda: &Pubkey,
    white_profile_pda: &Pubkey,
    escrow_pda: &Pubkey,
    game_id: u64,
) -> solana_sdk::instruction::Instruction {
    let discriminator: [u8; 8] = [0x4e, 0x8a, 0x11, 0xcc, 0x75, 0x3b, 0x9f, 0x28];

    let mut data = Vec::with_capacity(16);
    data.extend_from_slice(&discriminator);
    data.extend_from_slice(&game_id.to_le_bytes());

    let accounts = vec![
        AccountMeta::new(*session_pda, false),
        AccountMeta::new_readonly(*session_signer, true),
        AccountMeta::new_readonly(*player, false),
        AccountMeta::new(*game_pda, false),
        AccountMeta::new_readonly(*player_profile_pda, false),
        AccountMeta::new_readonly(*white_profile_pda, false),
        AccountMeta::new(*escrow_pda, false),
        AccountMeta::new_readonly(system_program::id(), false),
    ];

    solana_sdk::instruction::Instruction { program_id: *program_id, accounts, data }
}

/// Build a transaction: `init_profile` (if needed) + `authorize_global_session`
/// — the player sees **one wallet popup ever**.
pub fn build_first_time_auth_tx(
    program_id: &Pubkey,
    player: &Pubkey,
    session_key: Pubkey,
    deposit_lamports: u64,
) -> (Transaction, Pubkey) {
    let (session_pda, _bump) = find_global_session_pda(program_id, player);
    let authorize_ix = build_authorize_global_session_ix(
        program_id,
        player,
        &session_pda,
        AuthorizeGlobalSessionArgs {
            session_key,
            duration_secs: None,
            spending_limit: None,
            max_wager: None,
            games: None,
            deposit_lamports,
        },
    );
    let tx = Transaction::new_with_payer(&[authorize_ix], Some(player));
    (tx, session_pda)
}

// ── Borsh serialization helpers ───────────────────────────────────────────────

fn push_option_i64(buf: &mut Vec<u8>, v: Option<i64>) {
    match v {
        Some(x) => { buf.push(1); buf.extend_from_slice(&x.to_le_bytes()); }
        None => buf.push(0),
    }
}

fn push_option_u64(buf: &mut Vec<u8>, v: Option<u64>) {
    match v {
        Some(x) => { buf.push(1); buf.extend_from_slice(&x.to_le_bytes()); }
        None => buf.push(0),
    }
}

fn push_option_u16(buf: &mut Vec<u8>, v: Option<u16>) {
    match v {
        Some(x) => { buf.push(1); buf.extend_from_slice(&x.to_le_bytes()); }
        None => buf.push(0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_session_pda_deterministic() {
        let prog = Pubkey::new_unique();
        let player = Pubkey::new_unique();
        let (a, ba) = find_global_session_pda(&prog, &player);
        let (b, bb) = find_global_session_pda(&prog, &player);
        assert_eq!(a, b);
        assert_eq!(ba, bb);
    }

    #[test]
    fn different_players_different_pdas() {
        let prog = Pubkey::new_unique();
        let (a, _) = find_global_session_pda(&prog, &Pubkey::new_unique());
        let (b, _) = find_global_session_pda(&prog, &Pubkey::new_unique());
        assert_ne!(a, b);
    }
}
