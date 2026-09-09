use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use super::{GAME_SEED, SESSION_DELEGATION_SEED};

const PARTICIPANTS_CACHE_TTL: Duration = Duration::from_secs(300);

#[derive(Clone, Copy)]
struct CachedParticipants {
    white: Pubkey,
    black: Pubkey,
    cached_at: Instant,
}

#[derive(Clone)]
pub struct GameParticipantsCache {
    rpc: Arc<RpcClient>,
    program_id: Pubkey,
    cache: Arc<Mutex<HashMap<u64, CachedParticipants>>>,
    #[cfg(test)]
    session_keys: Arc<Mutex<HashMap<(u64, Pubkey), Pubkey>>>,
}

impl GameParticipantsCache {
    pub fn new(rpc: Arc<RpcClient>, program_id: Pubkey) -> Self {
        Self {
            rpc,
            program_id,
            cache: Arc::new(Mutex::new(HashMap::new())),
            #[cfg(test)]
            session_keys: Arc::new(Mutex::new(HashMap::new())),
        }
    }

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

    #[cfg(test)]
    pub fn seed_session_for_test(&self, game_id: u64, wallet: Pubkey, session_key: Pubkey) {
        self.session_keys
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert((game_id, wallet), session_key);
    }

    pub async fn session_key_authorized(
        &self,
        game_id: u64,
        wallet: &Pubkey,
        claimed_session_key: &Pubkey,
    ) -> bool {
        #[cfg(test)]
        if self
            .session_keys
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&(game_id, *wallet))
            == Some(claimed_session_key)
        {
            return true;
        }

        is_session_key_authorized(
            &self.rpc,
            &self.program_id,
            game_id,
            wallet,
            claimed_session_key,
        )
        .await
    }

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

pub async fn is_session_key_authorized(
    rpc: &Arc<RpcClient>,
    program_id: &Pubkey,
    game_id: u64,
    wallet: &Pubkey,
    claimed_session_key: &Pubkey,
) -> bool {
    let (pda, _bump) = Pubkey::find_program_address(
        &[
            SESSION_DELEGATION_SEED,
            &game_id.to_le_bytes(),
            wallet.as_ref(),
        ],
        program_id,
    );
    let rpc = Arc::clone(rpc);
    let Ok(Ok(data)) = tokio::task::spawn_blocking(move || rpc.get_account_data(&pda)).await else {
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

    #[tokio::test]
    async fn unknown_game_with_unreachable_rpc_resolves_to_none() {
        let cache = seeded_cache();
        assert!(cache.get(999).await.is_none());
    }
}
