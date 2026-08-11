//! Global persistent session key management for client-side use.
//!
//! One session keypair per wallet, persisted on disk, good for 30 days / 200
//! games. After `authorize_global_session` succeeds the VPS can co-sign every
//! `global_create_game` / `global_join_game` without another wallet popup.
//!
//! Storage: `<data-dir>/global_session_key.enc` (AES-256-GCM).

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose, Engine as _};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_sdk::{
    instruction::AccountMeta,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
#[allow(deprecated)]
use solana_system_interface::program as system_program;
use std::path::PathBuf;
use std::sync::Arc;

/// PDA seed prefix matching [`GlobalSessionDelegation::SEED`] on-chain.
const SEED: &[u8] = b"global_session";

// ── PDA helper ────────────────────────────────────────────────────────────────

/// Derive the `GlobalSessionDelegation` PDA for `player`.
pub fn find_global_session_pda(program_id: &Pubkey, player: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[SEED, player.as_ref()], program_id)
}

/// Reads the on-chain `GlobalSessionDelegation` PDA (if any) and reports
/// whether the program would currently reject a plain `authorize_global_session`
/// with `GlobalSessionAlreadyActive` — i.e. whether a revoke must happen
/// first. Mirrors the exact condition the program checks
/// (`programs/xfchess-game/src/account_ix/global_session_ix.rs`):
/// `enabled && now < expires_at && games_remaining > 0`, which is also
/// `GlobalSessionDelegation::is_valid` (`state/global_session.rs`).
///
/// This is a pre-flight optimization, not a correctness requirement: if the
/// account can't be read (doesn't exist yet, RPC hiccup, unexpected layout),
/// this returns `false` ("safe to authorize directly") and the caller's
/// existing reactive fallback — attempt authorize, revoke-and-retry on a
/// real `GlobalSessionAlreadyActive` error — still covers it. What this
/// avoids is the *common* case costing a wasted, doomed-to-fail first wallet
/// popup before falling back to revoke+reauthorize.
///
/// Account layout (Anchor `#[account]`, see `GlobalSessionDelegation`):
/// `[0..8)` disc, `[8..40)` player, `[40..72)` session_key,
/// `[72..80)` expires_at `i64` LE, `[80..88)` spending_limit `u64`,
/// `[88..96)` total_spent `u64`, `[96..104)` max_wager `u64`,
/// `[104..106)` games_remaining `u16` LE, `[106]` enabled `bool`, `[107]` bump.
pub fn global_session_is_live_onchain(rpc_url: &str, session_pda: &Pubkey) -> bool {
    use solana_client::rpc_client::RpcClient;
    use solana_commitment_config::CommitmentConfig;

    const MIN_LEN: usize = 108;

    let rpc = RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());
    let Ok(data) = rpc.get_account_data(session_pda) else {
        return false;
    };
    if data.len() < MIN_LEN {
        return false;
    }

    let expires_at = i64::from_le_bytes(data[72..80].try_into().unwrap_or_default());
    let games_remaining = u16::from_le_bytes(data[104..106].try_into().unwrap_or_default());
    let enabled = data[106] != 0;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    enabled && now < expires_at && games_remaining > 0
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

/// Appends an `instance_<port>` suffix when `wallet_port` is set to
/// something other than the default `7454`, so two same-machine instances
/// (e.g. `just dev2`'s P1/P2, which set `XFCHESS_WALLET_PORT` differently)
/// never resolve to the same directory. Takes the port as a plain argument
/// rather than reading the env var itself so the namespacing logic can be
/// unit-tested without mutating process-global state.
fn instance_scoped_dir(base: PathBuf, wallet_port: Option<&str>) -> PathBuf {
    match wallet_port.map(str::trim).filter(|p| !p.is_empty()) {
        Some(p) if p != "7454" => base.join(format!("instance_{p}")),
        _ => base,
    }
}

impl GlobalSessionKeyManager {
    /// Namespaced by `XFCHESS_WALLET_PORT` when set to a non-default value —
    /// see `integration::systems::get_hot_wallet_path`'s doc comment for why
    /// two same-machine instances must not share this directory.
    fn storage_dir() -> PathBuf {
        let base = directories::ProjectDirs::from("com", "xfchess", "XFChess")
            .map(|dirs| dirs.data_local_dir().to_path_buf())
            .unwrap_or_else(|| std::env::temp_dir().join("XFChess"));
        instance_scoped_dir(base, std::env::var("XFCHESS_WALLET_PORT").ok().as_deref())
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
        let json = serde_json::to_string(&data).map_err(|e| format!("serialize: {e}"))?;
        let cipher =
            Aes256Gcm::new_from_slice(&self.encryption_key).map_err(|e| format!("cipher: {e}"))?;
        #[allow(deprecated)]
        let nonce = Nonce::from_slice(b"xfchess glob"); // 12 bytes
        let encrypted = cipher
            .encrypt(nonce, json.as_bytes())
            .map_err(|e| format!("encrypt: {e}"))?;
        let encoded = general_purpose::STANDARD.encode(encrypted);
        std::fs::create_dir_all(&self.data_dir).map_err(|e| format!("mkdir: {e}"))?;
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
        let encoded = std::fs::read_to_string(&path).map_err(|e| format!("read: {e}"))?;
        let encrypted = general_purpose::STANDARD
            .decode(encoded)
            .map_err(|e| format!("base64: {e}"))?;
        let key = Self::derive_key(wallet);
        let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| format!("cipher: {e}"))?;
        #[allow(deprecated)]
        let nonce = Nonce::from_slice(b"xfchess glob");
        let decrypted = cipher
            .decrypt(nonce, encrypted.as_ref())
            .map_err(|e| format!("decrypt: {e}"))?;
        let data: GlobalSessionKeyData =
            serde_json::from_slice(&decrypted).map_err(|e| format!("deserialize: {e}"))?;
        if data.wallet_pubkey != wallet.to_string() {
            return Err("wallet mismatch".into());
        }
        if data.expires_at < Utc::now().timestamp() {
            return Err("expired".into());
        }
        let bytes = bs58::decode(&data.session_private_key)
            .into_vec()
            .map_err(|e| format!("decode key: {e}"))?;
        let kp = Keypair::try_from(bytes.as_slice()).map_err(|e| format!("keypair: {e}"))?;
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

/// Anchor's instruction discriminator: `sha256("global:<name>")[..8]`.
/// Computed at call time rather than hardcoded so it can never drift from the
/// on-chain instruction name — a hand-transcribed constant here previously
/// didn't match any real discriminator, so every session/global-game
/// instruction landed on the program's fallback handler
/// (`InstructionFallbackNotFound`) no matter what else was correct.
fn anchor_discriminator(name: &str) -> [u8; 8] {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(format!("global:{name}").as_bytes());
    let mut discriminator = [0u8; 8];
    discriminator.copy_from_slice(&hash[..8]);
    discriminator
}

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
pub fn build_authorize_global_session_ix(
    program_id: &Pubkey,
    player: &Pubkey,
    session_pda: &Pubkey,
    args: AuthorizeGlobalSessionArgs,
) -> solana_sdk::instruction::Instruction {
    let discriminator = anchor_discriminator("authorize_global_session");

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

    solana_sdk::instruction::Instruction {
        program_id: *program_id,
        accounts,
        data,
    }
}

/// Build a `revoke_global_session` instruction.
pub fn build_revoke_global_session_ix(
    program_id: &Pubkey,
    player: &Pubkey,
    session_pda: &Pubkey,
) -> solana_sdk::instruction::Instruction {
    let discriminator = anchor_discriminator("revoke_global_session");

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

/// Build a `withdraw_global_session` instruction — reclaims whatever balance
/// sits above the vault's own rent-exempt minimum, back to `player`. Safe to
/// call regardless of `enabled`/expiry state; it only touches lamports, not
/// the delegation fields. Used before a revoke+reauthorize cycle so a stale
/// session's leftover deposit isn't silently stranded when a fresh one is
/// created (see `establish_global_session`'s doc comment).
pub fn build_withdraw_global_session_ix(
    program_id: &Pubkey,
    player: &Pubkey,
    session_pda: &Pubkey,
) -> solana_sdk::instruction::Instruction {
    let discriminator = anchor_discriminator("withdraw_global_session");

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
    platform_fee: u64,
    base_time_seconds: u64,
    increment_seconds: u16,
) -> solana_sdk::instruction::Instruction {
    let discriminator = anchor_discriminator("global_create_game");

    let mut data = Vec::with_capacity(128);
    data.extend_from_slice(&discriminator);
    data.extend_from_slice(&game_id.to_le_bytes());
    data.extend_from_slice(&wager_amount.to_le_bytes());
    data.push(match_type);
    // platform_fee: u64 LE (universal fee from live SOL/GBP rate, replaces country String)
    data.extend_from_slice(&platform_fee.to_le_bytes());
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

    solana_sdk::instruction::Instruction {
        program_id: *program_id,
        accounts,
        data,
    }
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
    let discriminator = anchor_discriminator("global_join_game");

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

    solana_sdk::instruction::Instruction {
        program_id: *program_id,
        accounts,
        data,
    }
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
        Some(x) => {
            buf.push(1);
            buf.extend_from_slice(&x.to_le_bytes());
        }
        None => buf.push(0),
    }
}

fn push_option_u64(buf: &mut Vec<u8>, v: Option<u64>) {
    match v {
        Some(x) => {
            buf.push(1);
            buf.extend_from_slice(&x.to_le_bytes());
        }
        None => buf.push(0),
    }
}

fn push_option_u16(buf: &mut Vec<u8>, v: Option<u16>) {
    match v {
        Some(x) => {
            buf.push(1);
            buf.extend_from_slice(&x.to_le_bytes());
        }
        None => buf.push(0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Guards against the discriminator bug this file previously had: these 4
    /// instructions are hand-encoded (not via Anchor's generated client) so
    /// nothing else catches a wrong discriminator except the wallet rejecting
    /// the transaction on-chain at runtime. Compare against
    /// `xfchess_game::instruction::*::data()`, Anchor's own encoding, which is
    /// authoritative.
    #[test]
    fn discriminators_match_anchor_generated_encoding() {
        use anchor_lang::InstructionData;

        let program_id = Pubkey::new_unique();
        let player = Pubkey::new_unique();
        let session_pda = Pubkey::new_unique();
        let session_signer = Pubkey::new_unique();
        let game_pda = Pubkey::new_unique();
        let escrow_pda = Pubkey::new_unique();
        let player_profile_pda = Pubkey::new_unique();
        let white_profile_pda = Pubkey::new_unique();

        let authorize_args = AuthorizeGlobalSessionArgs {
            session_key: Pubkey::new_unique(),
            duration_secs: None,
            spending_limit: None,
            max_wager: None,
            games: None,
            deposit_lamports: 123,
        };
        let ix = build_authorize_global_session_ix(
            &program_id,
            &player,
            &session_pda,
            authorize_args.clone(),
        );
        let expected = xfchess_game::instruction::AuthorizeGlobalSession {
            args: xfchess_game::AuthorizeGlobalSessionArgs {
                session_key: authorize_args.session_key,
                duration_secs: authorize_args.duration_secs,
                spending_limit: authorize_args.spending_limit,
                max_wager: authorize_args.max_wager,
                games: authorize_args.games,
                deposit_lamports: authorize_args.deposit_lamports,
            },
        }
        .data();
        assert_eq!(ix.data, expected);

        let ix = build_revoke_global_session_ix(&program_id, &player, &session_pda);
        let expected = xfchess_game::instruction::RevokeGlobalSession {}.data();
        assert_eq!(ix.data, expected);

        // MatchType::Rated == discriminant 1 — build_global_create_game_ix takes
        // the raw discriminant byte, so this must match the enum's real encoding.
        let ix = build_global_create_game_ix(
            &program_id,
            &session_pda,
            &session_signer,
            &player,
            &game_pda,
            &escrow_pda,
            1,
            2,
            1,
            4,
            5,
            6,
        );
        let expected = xfchess_game::instruction::GlobalCreateGame {
            game_id: 1,
            wager_amount: 2,
            match_type: xfchess_game::state::MatchType::Rated,
            platform_fee: 4,
            base_time_seconds: 5,
            increment_seconds: 6,
        }
        .data();
        assert_eq!(ix.data, expected);

        let ix = build_global_join_game_ix(
            &program_id,
            &session_pda,
            &session_signer,
            &player,
            &game_pda,
            &player_profile_pda,
            &white_profile_pda,
            &escrow_pda,
            7,
        );
        let expected = xfchess_game::instruction::GlobalJoinGame { game_id: 7 }.data();
        assert_eq!(ix.data, expected);
    }

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

    /// Guards the `dev2` P1/P2 instance-isolation fix: two windows on one
    /// machine must not silently share this directory (that was the "logout
    /// logs me in as the other instance" bug).
    #[test]
    fn storage_dir_diverges_for_a_non_default_instance_port() {
        let base = PathBuf::from("/base");
        let p1 = instance_scoped_dir(base.clone(), None);
        let p2 = instance_scoped_dir(base.clone(), Some("7464"));
        assert_eq!(p1, base, "unset port must keep the pre-fix default path");
        assert_ne!(p1, p2, "a non-default port must get its own directory");
    }

    #[test]
    fn storage_dir_treats_explicit_default_port_same_as_unset() {
        let base = PathBuf::from("/base");
        let unset = instance_scoped_dir(base.clone(), None);
        let explicit_default = instance_scoped_dir(base.clone(), Some("7454"));
        let blank = instance_scoped_dir(base.clone(), Some("  "));
        assert_eq!(unset, explicit_default);
        assert_eq!(unset, blank);
    }
}
