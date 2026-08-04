//! On-chain source-of-truth for a game's registered participants.
//!
//! Closes the roster-building race in `game_log.rs`'s participant check
//! (previously trusted "first two `SessionInfo` posts seen," which a forged
//! post could win) by verifying a claimant against the `Game` account's
//! `white`/`black` wallets and the per-game `SessionDelegation` PDA — both
//! only that wallet's own on-chain authority can set. See
//! `specs/BraidChain.tla`'s `OnlyParticipantsAccepted` for the formal
//! property this closes, and `docs/plans/networking-hardening-plan.md` for
//! the broader audit this came out of.

use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use super::{GAME_SEED, SESSION_DELEGATION_SEED};

/// Games don't change participants mid-match, so a several-minute TTL is
/// safe and keeps this off the RPC hot path — the cache is only consulted
/// again once a new (unrecognized) pubkey shows up in a game's traffic.
const PARTICIPANTS_CACHE_TTL: Duration = Duration::from_secs(300);

#[derive(Clone, Copy)]
struct CachedParticipants {
    white: Pubkey,
    black: Pubkey,
    cached_at: Instant,
}

/// Cached, on-chain-verified `(white, black)` lookups keyed by `game_id`.
#[derive(Clone)]
pub struct GameParticipantsCache {
    rpc: Arc<RpcClient>,
    program_id: Pubkey,
    cache: Arc<Mutex<HashMap<u64, CachedParticipants>>>,
}

impl GameParticipantsCache {
    pub fn new(rpc: Arc<RpcClient>, program_id: Pubkey) -> Self {
        Self {
            rpc,
            program_id,
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Test-only: pre-seed the cache as if an on-chain lookup already
    /// confirmed `(white, black)` for `game_id`, without ever making a real
    /// RPC call. Lets tests exercise `check_participant`'s on-chain-verified
    /// path deterministically and offline.
    #[cfg(test)]
    pub fn seed_for_test(&self, game_id: u64, white: Pubkey, black: Pubkey) {
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        cache.insert(
            game_id,
            CachedParticipants {
                white,
                black,
                cached_at: Instant::now(),
            },
        );
    }

    /// Returns `(white, black)` for `game_id` — from cache if fresh,
    /// otherwise a fresh on-chain read. `None` if the game_id doesn't
    /// resolve to a real `Game` account (e.g. a casual/no-wallet game,
    /// which never had one).
    pub async fn get(&self, game_id: u64) -> Option<(Pubkey, Pubkey)> {
        {
            let cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(c) = cache.get(&game_id) {
                if c.cached_at.elapsed() < PARTICIPANTS_CACHE_TTL {
                    return Some((c.white, c.black));
                }
            }
        }

        let (white, black) = fetch_game_participants(&self.rpc, &self.program_id, game_id).await?;

        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        cache.insert(
            game_id,
            CachedParticipants {
                white,
                black,
                cached_at: Instant::now(),
            },
        );
        Some((white, black))
    }
}

/// Reads just `white`/`black` from the `Game` PDA — a minimal, focused parse
/// (not the full layout `settlement_worker::parse_game_account` walks, which
/// needs many more fields for its own purpose) since that's all a
/// participant check needs.
///
/// Layout: 8-byte Anchor discriminator, `game_id: u64` (8), `white: Pubkey`
/// (32), `black: Pubkey` (32) — see `programs/xfchess-game/src/state/game.rs`.
async fn fetch_game_participants(
    rpc: &Arc<RpcClient>,
    program_id: &Pubkey,
    game_id: u64,
) -> Option<(Pubkey, Pubkey)> {
    let (game_pda, _bump) =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], program_id);
    let rpc = Arc::clone(rpc);
    let data = tokio::task::spawn_blocking(move || rpc.get_account_data(&game_pda))
        .await
        .ok()?
        .ok()?;

    let white = Pubkey::try_from(data.get(16..48)?).ok()?;
    let black = Pubkey::try_from(data.get(48..80)?).ok()?;
    Some((white, black))
}

/// Verifies `claimed_session_key` is the real, currently-enabled session key
/// delegated on-chain for `wallet` in this specific `game_id`. The PDA this
/// reads can only have been created/updated by `wallet`'s own on-chain
/// authority (`authorize_session_key`), so this is unforgeable without
/// controlling that wallet — the same guarantee `moves_ix/record.rs`'s
/// `RecordMove` instruction already rests its own authorization on.
///
/// Layout: 8-byte discriminator, `game_id: u64` (8), `player: Pubkey` (32),
/// `session_key: Pubkey` (32), `expires_at: i64` (8), `max_batch_len: u16`
/// (2), `enabled: bool` (1), `bump: u8` (1) — see
/// `programs/xfchess-game/src/state/game.rs`'s `SessionDelegation`.
pub async fn is_session_key_authorized(
    rpc: &Arc<RpcClient>,
    program_id: &Pubkey,
    game_id: u64,
    wallet: &Pubkey,
    claimed_session_key: &Pubkey,
) -> bool {
    let (pda, _bump) = Pubkey::find_program_address(
        &[SESSION_DELEGATION_SEED, &game_id.to_le_bytes(), wallet.as_ref()],
        program_id,
    );
    let rpc = Arc::clone(rpc);
    let Ok(Ok(data)) = tokio::task::spawn_blocking(move || rpc.get_account_data(&pda)).await
    else {
        return false;
    };

    let Some(on_chain_key) = data.get(48..80).and_then(|b| Pubkey::try_from(b).ok()) else {
        return false;
    };
    let enabled = data.get(90).map(|b| *b != 0).unwrap_or(false);

    &on_chain_key == claimed_session_key && enabled
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seeded_cache() -> GameParticipantsCache {
        let rpc = Arc::new(crate::signing::solana::rpc::make_rpc("http://127.0.0.1:1"));
        GameParticipantsCache::new(rpc, Pubkey::new_unique())
    }

    /// The exact round-trip `GET /game/:id/participants` (`game_log.rs`)
    /// depends on: a seeded (cached) entry is returned without ever
    /// touching the RPC client, in the same order it was seeded — this is
    /// what lets Phase B/C tests stay offline and deterministic.
    #[tokio::test]
    async fn seeded_entry_is_returned_from_cache() {
        let cache = seeded_cache();
        let white = Pubkey::new_unique();
        let black = Pubkey::new_unique();
        cache.seed_for_test(42, white, black);

        let (got_white, got_black) = cache.get(42).await.expect("seeded entry must be found");
        assert_eq!(got_white, white);
        assert_eq!(got_black, black);
    }

    /// A `game_id` with no cache entry and an unreachable RPC (nothing ever
    /// bound on 127.0.0.1:1) must resolve to `None`, not panic or hang —
    /// this is the exact path a casual (no-wallet) game's `game_id` takes,
    /// since it never resolves to a real on-chain `Game` account either.
    #[tokio::test]
    async fn unknown_game_with_unreachable_rpc_resolves_to_none() {
        let cache = seeded_cache();
        assert!(cache.get(999).await.is_none());
    }
}
