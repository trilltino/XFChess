use parking_lot::Mutex;
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::{
    collections::HashMap,
    str::FromStr,
    sync::Arc,
    time::{Duration, Instant},
};
use tracing::{info, warn};

/// Ratings are on-chain fixed-point `u32` in hundredths of a rating point and are exposed here
/// as `f64` in the same unit (1500.00 is `150000.0`); callers divide by 100 for display.
#[derive(Clone, Debug)]
pub struct CachedElo {
    pub elo_rating: f64,
    pub country: String,
    pub username: String,
    pub lichess_verified: bool,
    pub lichess_blitz: u32,
    pub lichess_last_sync: i64,
    pub seeded_from_external: bool,
    pub external_elo_source: u8,
    pub elo_bullet: f64,
    pub elo_blitz: f64,
    pub elo_rapid: f64,
    pub cached_at: Instant,
}

/// Byte offsets in a serialized `PlayerProfile`, discriminator included. Every fixed-size field
/// precedes the three strings, so these offsets are the same for every account. The layout test
/// below serializes the program's own struct and checks each one.
mod offset {
    pub const ELO_CLASSICAL: usize = 56;
    pub const ELO_BULLET: usize = 60;
    pub const ELO_BLITZ: usize = 64;
    pub const ELO_RAPID: usize = 68;
    pub const LICHESS_VERIFIED: usize = 183;
    pub const LICHESS_BLITZ: usize = 184;
    pub const LICHESS_LAST_SYNC: usize = 196;
    pub const EXTERNAL_ELO_SOURCE: usize = 204;
    pub const SEEDED_FROM_EXTERNAL: usize = 205;
    /// `country`, `username`, `lichess_username`: each a `u32` length then UTF-8 bytes.
    pub const STRINGS: usize = 206;
}

/// The default a player has before their first rated game.
const INITIAL_RATING: f64 = 120_000.0;

fn read_u32(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(data.get(at..at.checked_add(4)?)?.try_into().ok()?))
}

fn read_i64(data: &[u8], at: usize) -> Option<i64> {
    Some(i64::from_le_bytes(data.get(at..at.checked_add(8)?)?.try_into().ok()?))
}

/// A length-prefixed string at `at`, and the offset just past it.
fn read_string(data: &[u8], at: usize) -> Option<(String, usize)> {
    let len = read_u32(data, at)? as usize;
    let start = at.checked_add(4)?;
    let end = start.checked_add(len)?;
    let text = String::from_utf8(data.get(start..end)?.to_vec()).ok()?;
    Some((text, end))
}

/// An unplayed bucket (0) counts as the initial rating, as the program treats it.
fn rating_at(data: &[u8], at: usize) -> f64 {
    match read_u32(data, at) {
        Some(0) | None => INITIAL_RATING,
        Some(value) => f64::from(value),
    }
}

fn decode_profile(data: &[u8]) -> Result<CachedElo, String> {
    if data.len() < offset::STRINGS {
        return Err("Player profile is shorter than the current layout".to_string());
    }
    let (country, next) = read_string(data, offset::STRINGS).unwrap_or_default();
    let username = read_string(data, next)
        .map(|(name, _)| name)
        .unwrap_or_default();
    Ok(CachedElo {
        elo_rating: rating_at(data, offset::ELO_CLASSICAL),
        country,
        username,
        lichess_verified: data[offset::LICHESS_VERIFIED] != 0,
        lichess_blitz: read_u32(data, offset::LICHESS_BLITZ).unwrap_or(0),
        lichess_last_sync: read_i64(data, offset::LICHESS_LAST_SYNC).unwrap_or(0),
        seeded_from_external: data[offset::SEEDED_FROM_EXTERNAL] != 0,
        external_elo_source: data[offset::EXTERNAL_ELO_SOURCE],
        elo_bullet: rating_at(data, offset::ELO_BULLET),
        elo_blitz: rating_at(data, offset::ELO_BLITZ),
        elo_rapid: rating_at(data, offset::ELO_RAPID),
        cached_at: Instant::now(),
    })
}

#[derive(Clone)]
pub struct EloCache {
    rpc: Arc<RpcClient>,
    cache: Arc<Mutex<HashMap<String, CachedElo>>>,
    ttl: Duration,
    program_id: Pubkey,
}

impl EloCache {
    pub fn new(
        rpc_url: String,
        ttl: Duration,
        program_id: Pubkey,
    ) -> solana_client::client_error::Result<Self> {
        // Use the shared hardened constructor (bounded request timeout) rather than a
        // bespoke client — an RPC hang must not wedge profile/matchmaking reads.
        let rpc = Arc::new(crate::signing::solana::make_rpc(&rpc_url)?);

        Ok(Self {
            rpc,
            cache: Arc::new(Mutex::new(HashMap::new())),
            ttl,
            program_id,
        })
    }

    pub async fn get_elo(&self, pubkey: &str) -> Result<CachedElo, String> {
        // Check cache first
        {
            let cache = self.cache.lock();
            if let Some(cached) = cache.get(pubkey) {
                if cached.cached_at.elapsed() < self.ttl {
                    return Ok(cached.clone());
                }
            }
        }

        // Cache miss or expired - fetch from on-chain
        self.fetch_elo(pubkey).await
    }

    async fn fetch_elo(&self, pubkey: &str) -> Result<CachedElo, String> {
        let pk = Pubkey::from_str(pubkey).map_err(|e| format!("Invalid pubkey: {}", e))?;

        // Derive PlayerProfile PDA
        let (profile_pda, _bump) =
            Pubkey::find_program_address(&[b"profile", pk.as_ref()], &self.program_id);

        let account = self
            .rpc
            .get_account(&profile_pda)
            .map_err(|e| format!("Failed to fetch profile: {}", e))?;

        if account.data.is_empty() {
            return Err("Player profile not found".to_string());
        }

        let cached = decode_profile(&account.data)?;

        {
            let mut cache = self.cache.lock();
            cache.insert(pubkey.to_string(), cached.clone());
        }

        info!(
            "[ELO_CACHE] Fetched profile for {}: {} ELO, country: {}, username: {}",
            pubkey, cached.elo_rating, cached.country, cached.username
        );

        Ok(cached)
    }

    pub async fn batch_get_elo(&self, pubkeys: &[String]) -> HashMap<String, CachedElo> {
        let mut results = HashMap::new();

        for pubkey in pubkeys {
            match self.get_elo(pubkey).await {
                Ok(elo) => {
                    results.insert(pubkey.clone(), elo);
                }
                Err(e) => {
                    warn!("[EloCache] Failed to fetch ELO for {}: {}", pubkey, e);
                }
            }
        }

        results
    }

    pub fn invalidate(&self, pubkey: &str) {
        let mut cache = self.cache.lock();
        cache.remove(pubkey);
        info!("[EloCache] Invalidated cache for {}", pubkey);
    }

    pub fn clear(&self) {
        let mut cache = self.cache.lock();
        cache.clear();
        info!("[EloCache] Cleared all cache entries");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anchor_lang::AccountSerialize;
    use anyhow::Context as _;
    use xfchess_game::state::PlayerProfile;

    fn serialized(profile: &PlayerProfile) -> anyhow::Result<Vec<u8>> {
        let mut bytes = Vec::new();
        profile
            .try_serialize(&mut bytes)
            .context("serialize the program profile")?;
        Ok(bytes)
    }

    fn profile(country: &str, username: &str, lichess_username: &str) -> PlayerProfile {
        PlayerProfile {
            authority: Pubkey::new_unique(),
            wins: 11,
            games_played: 12,
            elo_rating: 151_200,
            elo_bullet: 130_100,
            elo_blitz: 141_300,
            elo_rapid: 0,
            games_blitz: 7,
            last_rated_opponent: [9; 8],
            repeat_streak: 3,
            last_played: 1_700_000_000,
            total_wagered: 5_000_000,
            date_of_birth: 631_152_000,
            is_verified: true,
            annual_wins_eur: 77,
            username_set: true,
            lichess_verified: true,
            lichess_blitz: 163_000,
            lichess_rapid: 150_000,
            lichess_bullet: 140_000,
            lichess_last_sync: 1_699_999_999,
            external_elo_source: 1,
            seeded_from_external: true,
            country: country.into(),
            username: username.into(),
            lichess_username: lichess_username.into(),
            ..Default::default()
        }
    }

    #[test]
    fn offsets_match_the_program_layout_for_any_string_lengths() -> anyhow::Result<()> {
        for (country, username, lichess) in [
            ("GB", "xfplayer", "lichess_name"),
            ("", "", ""),
            ("DE", "twenty_char_username", "thirty_character_lichess_name_"),
        ] {
            let bytes = serialized(&profile(country, username, lichess))?;
            let decoded = decode_profile(&bytes).map_err(anyhow::Error::msg)?;
            assert_eq!(decoded.elo_rating, 151_200.0);
            assert_eq!(decoded.elo_bullet, 130_100.0);
            assert_eq!(decoded.elo_blitz, 141_300.0);
            assert_eq!(decoded.elo_rapid, INITIAL_RATING, "an unplayed bucket is the default");
            assert!(decoded.lichess_verified);
            assert_eq!(decoded.lichess_blitz, 163_000);
            assert_eq!(decoded.lichess_last_sync, 1_699_999_999);
            assert_eq!(decoded.external_elo_source, 1);
            assert!(decoded.seeded_from_external);
            assert_eq!(decoded.country, country);
            assert_eq!(decoded.username, username);
        }
        Ok(())
    }

    #[test]
    fn a_default_profile_decodes_to_the_initial_ratings() -> anyhow::Result<()> {
        let bytes = serialized(&PlayerProfile::default())?;
        let decoded = decode_profile(&bytes).map_err(anyhow::Error::msg)?;
        assert_eq!(decoded.elo_rating, INITIAL_RATING);
        assert!(!decoded.lichess_verified && !decoded.seeded_from_external);
        assert!(decoded.country.is_empty() && decoded.username.is_empty());
        Ok(())
    }

    #[test]
    fn truncated_or_old_layout_data_is_rejected_not_misread() -> anyhow::Result<()> {
        let bytes = serialized(&profile("GB", "xf", ""))?;
        assert!(decode_profile(&bytes[..offset::STRINGS - 1]).is_err());
        assert!(decode_profile(&[]).is_err());
        Ok(())
    }
}
