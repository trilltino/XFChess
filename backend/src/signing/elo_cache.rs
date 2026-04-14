//! ELO caching system for on-chain player profile data.
//!
//! This module provides an in-memory cache of player ELO ratings
//! queried from on-chain PlayerProfile accounts. This enables
//! fast matchmaking without requiring clients to know their
//! current ELO rating.

use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    pubkey::Pubkey,
    commitment_config::CommitmentConfig,
};
use std::{
    collections::HashMap,
    str::FromStr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tracing::{info, warn};

/// Cached ELO data with timestamp.
#[derive(Clone, Debug)]
pub struct CachedElo {
    /// Player's ELO rating (f64 from Glicko-2)
    pub elo_rating: f64,
    /// Player's rating deviation
    pub rd: f64,
    /// Player's country code (ISO 3166-1 alpha-2)
    pub country: String,
    /// When this cache entry was last updated
    pub cached_at: Instant,
}

/// ELO cache for player profile data.
#[derive(Clone)]
pub struct EloCache {
    /// RPC client for querying on-chain data
    rpc: Arc<RpcClient>,
    /// Cache mapping pubkey to ELO data
    cache: Arc<Mutex<HashMap<String, CachedElo>>>,
    /// Cache TTL - entries expire after this duration
    ttl: Duration,
    /// Program ID for on-chain PlayerProfile accounts
    program_id: Pubkey,
}

impl EloCache {
    /// Creates a new ELO cache with the given RPC URL, TTL, and program ID.
    ///
    /// # Arguments
    /// * `rpc_url` - Solana RPC endpoint URL
    /// * `ttl` - Time-to-live for cache entries
    /// * `program_id` - Program ID for on-chain PlayerProfile accounts
    ///
    /// # Returns
    /// A new EloCache instance
    pub fn new(rpc_url: String, ttl: Duration, program_id: Pubkey) -> Self {
        let rpc = Arc::new(RpcClient::new_with_commitment(
            rpc_url,
            CommitmentConfig::confirmed(),
        ));
        
        Self {
            rpc,
            cache: Arc::new(Mutex::new(HashMap::new())),
            ttl,
            program_id,
        }
    }

    /// Gets a player's ELO rating from cache, refreshing if expired.
    ///
    /// # Arguments
    /// * `pubkey` - Player's wallet public key
    ///
    /// # Returns
    /// Cached ELO data, or error if query fails
    pub async fn get_elo(&self, pubkey: &str) -> Result<CachedElo, String> {
        // Check cache first
        {
            let cache = self.cache.lock().unwrap();
            if let Some(cached) = cache.get(pubkey) {
                if cached.cached_at.elapsed() < self.ttl {
                    return Ok(cached.clone());
                }
            }
        }

        // Cache miss or expired - fetch from on-chain
        self.fetch_elo(pubkey).await
    }

    /// Fetches ELO data from on-chain PlayerProfile account.
    ///
    /// # Arguments
    /// * `pubkey` - Player's wallet public key
    ///
    /// # Returns
    /// Cached ELO data
    async fn fetch_elo(&self, pubkey: &str) -> Result<CachedElo, String> {
        let pk = Pubkey::from_str(pubkey)
            .map_err(|e| format!("Invalid pubkey: {}", e))?;

        // Derive PlayerProfile PDA
        let (profile_pda, _bump) = Pubkey::find_program_address(
            &[b"profile", pk.as_ref()],
            &self.program_id,
        );

        // Fetch account from RPC
        let account = self.rpc
            .get_account(&profile_pda)
            .map_err(|e| format!("Failed to fetch profile: {}", e))?;

        if account.data.is_empty() {
            return Err("Player profile not found".to_string());
        }

        // Deserialize PlayerProfile
        // Note: This is a simplified deserialization. In production,
        // you would use the actual PlayerProfile struct from the program.
        let elo_rating = self.deserialize_f64(&account.data, 40)?; // Offset based on struct
        let rd = self.deserialize_f64(&account.data, 48)?;
        let country = self.deserialize_string(&account.data, 56, 2)?;

        let cached = CachedElo {
            elo_rating,
            rd,
            country: country.clone(),
            cached_at: Instant::now(),
        };

        // Update cache
        {
            let mut cache = self.cache.lock().unwrap();
            cache.insert(pubkey.to_string(), cached.clone());
        }

        info!("[ELO_CACHE] Fetched ELO for {}: {} (RD: {}, country: {})",
            pubkey, elo_rating, rd, country);

        Ok(cached)
    }

    /// Batch fetches ELO data for multiple players.
    ///
    /// # Arguments
    /// * `pubkeys` - List of player public keys
    ///
    /// # Returns
    /// Map of pubkey to ELO data
    pub async fn batch_get_elo(&self, pubkeys: &[String]) -> HashMap<String, CachedElo> {
        let mut results = HashMap::new();
        
        for pubkey in pubkeys {
            match self.get_elo(pubkey).await {
                Ok(elo) => { results.insert(pubkey.clone(), elo); }
                Err(e) => { warn!("[EloCache] Failed to fetch ELO for {}: {}", pubkey, e); }
            }
        }
        
        results
    }

    /// Invalidates cache entry for a specific player.
    ///
    /// # Arguments
    /// * `pubkey` - Player's wallet public key
    pub fn invalidate(&self, pubkey: &str) {
        let mut cache = self.cache.lock().unwrap();
        cache.remove(pubkey);
        info!("[EloCache] Invalidated cache for {}", pubkey);
    }

    /// Clears all cached entries.
    pub fn clear(&self) {
        let mut cache = self.cache.lock().unwrap();
        cache.clear();
        info!("[EloCache] Cleared all cache entries");
    }

    /// Deserializes an f64 from account data at the given offset.
    fn deserialize_f64(&self, data: &[u8], offset: usize) -> Result<f64, String> {
        if offset + 8 > data.len() {
            return Err("Offset out of bounds".to_string());
        }
        let bytes: [u8; 8] = data[offset..offset+8].try_into()
            .map_err(|_| "Failed to read bytes".to_string())?;
        Ok(f64::from_le_bytes(bytes))
    }

    /// Deserializes a String from account data at the given offset.
    fn deserialize_string(&self, data: &[u8], offset: usize, max_len: usize) -> Result<String, String> {
        if offset + max_len > data.len() {
            return Err("Offset out of bounds".to_string());
        }
        
        // Read length prefix (u32 in Anchor)
        let len_bytes: [u8; 4] = data[offset..offset+4].try_into()
            .map_err(|_| "Failed to read length".to_string())?;
        let len = u32::from_le_bytes(len_bytes) as usize;
        
        if len == 0 {
            return Ok(String::new());
        }
        
        let string_offset = offset + 4;
        if string_offset + len > data.len() {
            return Err("String data out of bounds".to_string());
        }
        
        String::from_utf8(data[string_offset..string_offset+len].to_vec())
            .map_err(|e| format!("Invalid UTF-8: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_f64() {
        let cache = EloCache::new("http://localhost".to_string(), Duration::from_secs(60));
        let mut data = vec![0u8; 56];
        
        // Write f64 = 1200.5 at offset 40
        let value: f64 = 1200.5;
        data[40..48].copy_from_slice(&value.to_le_bytes());
        
        let result = cache.deserialize_f64(&data, 40).unwrap();
        assert!((result - 1200.5).abs() < 0.01);
    }
}
