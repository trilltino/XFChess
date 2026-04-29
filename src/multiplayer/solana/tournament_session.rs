//! Tournament-scoped session key management for client-side use.
//!
//! Builds the `authorize_tournament_session` instruction and extends
//! [`SessionKeyManager`](crate::multiplayer::solana::session_key_manager::SessionKeyManager)
//! to key stored sessions by `(wallet, tournament_id)` instead of a global singleton.

use solana_sdk::{
    instruction::AccountMeta,
    pubkey::Pubkey,
    signature::Keypair,
    transaction::Transaction,
};
#[allow(deprecated)]
use solana_sdk::system_program;
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
    let discriminator: [u8; 8] = [
        0x6a, 0x1e, 0xd5, 0x3b, 0x8c, 0x2f, 0x9a, 0xe4,
    ];

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

/// Build a transaction that bundles `register_player` + `authorize_tournament_session`
/// so the player sees **one wallet popup** at registration time.
pub fn build_registration_with_session_tx(
    program_id: &Pubkey,
    tournament_id: u64,
    tournament_pubkey: &Pubkey,
    tournament_bump: u8,
    player_pubkey: &Pubkey,
    session_key: &Pubkey,
    deposit_lamports: u64,
) -> (Transaction, Pubkey) {
    // Derive session delegation PDA
    let (session_pda, _session_bump) =
        find_tournament_session_pda(program_id, tournament_id, player_pubkey);

    // register_player instruction
    let register_ix = build_register_player_ix(
        program_id,
        tournament_id,
        tournament_pubkey,
        player_pubkey,
    );

    // authorize_tournament_session instruction
    let authorize_ix = build_authorize_tournament_session_ix(
        program_id,
        tournament_id,
        tournament_pubkey,
        tournament_bump,
        &session_pda,
        player_pubkey,
        AuthorizeTournamentSessionArgs {
            session_key: *session_key,
            duration_secs: None,
            spending_limit: None,
            max_wager: None,
            deposit_lamports,
        },
    );

    let tx = Transaction::new_with_payer(
        &[register_ix, authorize_ix],
        Some(player_pubkey),
    );
    // Note: tx is unsigned; the wallet adapter must sign before sending.

    (tx, session_pda)
}

/// Build a `register_player` instruction (simplified).
fn build_register_player_ix(
    program_id: &Pubkey,
    tournament_id: u64,
    tournament_pubkey: &Pubkey,
    player_pubkey: &Pubkey,
) -> solana_sdk::instruction::Instruction {
    // Anchor discriminator for `register_player`
    let discriminator: [u8; 8] = [
        0x2e, 0x42, 0x6e, 0x0a, 0x7c, 0x44, 0x0e, 0x9a,
    ];

    let mut data = Vec::with_capacity(16);
    data.extend_from_slice(&discriminator);
    data.extend_from_slice(&tournament_id.to_le_bytes());

    let accounts = vec![
        AccountMeta::new(*tournament_pubkey, false),
        AccountMeta::new(*player_pubkey, true),
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
