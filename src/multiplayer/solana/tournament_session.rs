//! Tournament-scoped session key management for client-side use.
//!
//! Builds the `authorize_tournament_session` instruction and extends
//! [`SessionKeyManager`](crate::multiplayer::solana::session_key_manager::SessionKeyManager)
//! to key stored sessions by `(wallet, tournament_id)` instead of a global singleton.

#[allow(deprecated)]
use solana_system_interface::program as system_program;
use solana_sdk::{instruction::AccountMeta, pubkey::Pubkey, signature::Keypair};
use std::collections::HashMap;

/// PDA seed prefix matching the on-chain constant.
const SEED: &[u8] = b"tournament_session";

/// In-memory cache of tournament session keypairs keyed by `(wallet, tournament_id)`.
#[derive(Default)]
pub struct TournamentSessionCache {
    sessions: HashMap<(Pubkey, u64), Keypair>,
}

impl TournamentSessionCache {
    /// Get or create a session keypair for a specific tournament.
    pub fn get_or_create(&mut self, wallet: Pubkey, tournament_id: u64) -> &Keypair {
        self.sessions
            .entry((wallet, tournament_id))
            .or_insert_with(Keypair::new)
    }

    /// Retrieve an existing session keypair, if any.
    pub fn get(&self, wallet: &Pubkey, tournament_id: u64) -> Option<&Keypair> {
        self.sessions.get(&(*wallet, tournament_id))
    }

    /// Remove a session (e.g. after revocation).
    pub fn remove(&mut self, wallet: &Pubkey, tournament_id: u64) {
        self.sessions.remove(&(*wallet, tournament_id));
    }
}

/// Derive the `TournamentSessionDelegation` PDA for `(tournament_id, player)`.
pub fn find_tournament_session_pda(
    program_id: &Pubkey,
    tournament_id: u64,
    player: &Pubkey,
) -> (Pubkey, u8) {
    let tid_bytes = tournament_id.to_le_bytes();
    let seeds: &[&[u8]] = &[SEED, tid_bytes.as_ref(), player.as_ref()];
    Pubkey::find_program_address(seeds, program_id)
}

/// Derive the `Tournament` PDA.
pub fn find_tournament_pda(program_id: &Pubkey, tournament_id: u64) -> (Pubkey, u8) {
    let tid_bytes = tournament_id.to_le_bytes();
    let seeds: &[&[u8]] = &[b"tournament", tid_bytes.as_ref()];
    Pubkey::find_program_address(seeds, program_id)
}

/// Arguments for `authorize_tournament_session`, matching the on-chain struct.
#[derive(Debug, Clone)]
pub struct AuthorizeTournamentSessionArgs {
    pub session_key: Pubkey,
    pub duration_secs: Option<i64>,
    pub spending_limit: Option<u64>,
    pub max_wager: Option<u64>,
    pub deposit_lamports: u64,
}

/// Build an `authorize_tournament_session` instruction.
///
/// The caller must supply the program ID and all account pubkeys; this helper
/// does not perform on-chain lookups.
pub fn build_authorize_tournament_session_ix(
    program_id: &Pubkey,
    tournament_id: u64,
    tournament_pubkey: &Pubkey,
    _tournament_bump: u8,
    session_delegation_pubkey: &Pubkey,
    player_pubkey: &Pubkey,
    args: AuthorizeTournamentSessionArgs,
) -> solana_sdk::instruction::Instruction {
    // Anchor discriminator for `authorize_tournament_session`
    // sha256("global:authorize_tournament_session")[..8]
    let discriminator: [u8; 8] = [0x6a, 0x1e, 0xd5, 0x3b, 0x8c, 0x2f, 0x9a, 0xe4];

    let mut data = Vec::with_capacity(256);
    data.extend_from_slice(&discriminator);
    data.extend_from_slice(&tournament_id.to_le_bytes());
    // AuthorizeTournamentSessionArgs fields
    data.extend_from_slice(args.session_key.as_ref());
    // Option<i64> duration_secs
    if let Some(dur) = args.duration_secs {
        data.push(1);
        data.extend_from_slice(&dur.to_le_bytes());
    } else {
        data.push(0);
    }
    // Option<u64> spending_limit
    if let Some(limit) = args.spending_limit {
        data.push(1);
        data.extend_from_slice(&limit.to_le_bytes());
    } else {
        data.push(0);
    }
    // Option<u64> max_wager
    if let Some(wager) = args.max_wager {
        data.push(1);
        data.extend_from_slice(&wager.to_le_bytes());
    } else {
        data.push(0);
    }
    // u64 deposit_lamports
    data.extend_from_slice(&args.deposit_lamports.to_le_bytes());

    let accounts = vec![
        AccountMeta::new_readonly(*tournament_pubkey, false),
        AccountMeta::new(*session_delegation_pubkey, false),
        AccountMeta::new(*player_pubkey, true),
        AccountMeta::new_readonly(system_program::id(), false),
    ];

    solana_sdk::instruction::Instruction {
        program_id: *program_id,
        accounts,
        data,
    }
}

/// Number of `TournamentPlayersShard` PDAs actually initialized on-chain for
/// a tournament of this size (each shard holds up to 64 players). Must match
/// `programs/xfchess-game/src/tournament_ix/shards.rs::required_shards`.
fn required_shards(max_players: u16) -> u8 {
    match max_players {
        0..=64 => 1,
        65..=128 => 2,
        _ => 4,
    }
}

/// Derive the `PlayerProfile` PDA — must already exist (created at
/// first-time wallet setup / `init_profile`) before a player can register
/// for any tournament.
pub fn find_player_profile_pda(program_id: &Pubkey, player: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"profile", player.as_ref()], program_id)
}

/// Derive the tournament escrow PDA (holds the guaranteed prize plus, during
/// Registration, entry-fee deposits).
pub fn find_tournament_escrow_pda(program_id: &Pubkey, tournament_id: u64) -> (Pubkey, u8) {
    let tid_bytes = tournament_id.to_le_bytes();
    Pubkey::find_program_address(&[b"t_escrow", tid_bytes.as_ref()], program_id)
}

/// Derive shard `idx`'s PDA (0-3).
fn find_shard_pda(program_id: &Pubkey, tournament_id: u64, idx: u8) -> Pubkey {
    let tid_bytes = tournament_id.to_le_bytes();
    Pubkey::find_program_address(&[b"tourney_players", &[idx], tid_bytes.as_ref()], program_id).0
}

/// Build a real `register_player` instruction: this is the transaction that
/// actually deposits the entry fee into escrow and adds the player to their
/// tournament shard on-chain. Matches
/// `programs/xfchess-game/src/tournament_ix/registration/register.rs`'s
/// `RegisterPlayer` accounts struct exactly — shards past what `max_players`
/// requires are passed as the program ID, Anchor's sentinel for `None` on an
/// `Option<Account<..>>` field (same convention the backend's own
/// `cancel_tournament_ix`/`start_tournament_ix` use).
pub fn build_register_player_ix(
    program_id: &Pubkey,
    tournament_id: u64,
    max_players: u16,
    player_pubkey: &Pubkey,
    host_treasury: &Pubkey,
    elo: u32,
) -> solana_sdk::instruction::Instruction {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(b"global:register_player");
    let discriminator: [u8; 8] = hasher.finalize()[..8].try_into().expect("sha256 >= 8 bytes");

    let mut data = Vec::with_capacity(20);
    data.extend_from_slice(&discriminator);
    data.extend_from_slice(&tournament_id.to_le_bytes());
    data.extend_from_slice(&elo.to_le_bytes());

    let (tournament_pda, _) = find_tournament_pda(program_id, tournament_id);
    let (player_profile_pda, _) = find_player_profile_pda(program_id, player_pubkey);
    let (escrow_pda, _) = find_tournament_escrow_pda(program_id, tournament_id);
    let present = required_shards(max_players);
    let shard_meta = |idx: u8| {
        if idx < present {
            AccountMeta::new(find_shard_pda(program_id, tournament_id, idx), false)
        } else {
            AccountMeta::new_readonly(*program_id, false)
        }
    };

    let accounts = vec![
        AccountMeta::new(tournament_pda, false),
        AccountMeta::new_readonly(player_profile_pda, false),
        AccountMeta::new(*player_pubkey, true),
        AccountMeta::new(escrow_pda, false),
        shard_meta(0),
        shard_meta(1),
        shard_meta(2),
        shard_meta(3),
        AccountMeta::new(*host_treasury, false),
        AccountMeta::new_readonly(system_program::id(), false),
    ];

    solana_sdk::instruction::Instruction {
        program_id: *program_id,
        accounts,
        data,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::signature::Signer;

    #[test]
    fn tournament_session_pda_deterministic() {
        let program_id = Pubkey::new_unique();
        let player = Pubkey::new_unique();
        let (pda1, bump1) = find_tournament_session_pda(&program_id, 42, &player);
        let (pda2, bump2) = find_tournament_session_pda(&program_id, 42, &player);
        assert_eq!(pda1, pda2);
        assert_eq!(bump1, bump2);
    }

    #[test]
    fn different_tournaments_different_pdas() {
        let program_id = Pubkey::new_unique();
        let player = Pubkey::new_unique();
        let (pda1, _) = find_tournament_session_pda(&program_id, 1, &player);
        let (pda2, _) = find_tournament_session_pda(&program_id, 2, &player);
        assert_ne!(pda1, pda2);
    }

    #[test]
    fn cache_get_or_create_returns_same_keypair() {
        let mut cache = TournamentSessionCache::default();
        let wallet = Pubkey::new_unique();
        let kp1 = cache.get_or_create(wallet, 1).pubkey();
        let kp2 = cache.get_or_create(wallet, 1).pubkey();
        assert_eq!(kp1, kp2);
    }

    #[test]
    fn cache_remove_clears_entry() {
        let mut cache = TournamentSessionCache::default();
        let wallet = Pubkey::new_unique();
        cache.get_or_create(wallet, 1);
        assert!(cache.get(&wallet, 1).is_some());
        cache.remove(&wallet, 1);
        assert!(cache.get(&wallet, 1).is_none());
    }
}
