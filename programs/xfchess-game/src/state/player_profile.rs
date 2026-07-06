//! Account structure encompassing a player's long-term ranking and stats.

use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace, Default)]
pub struct PlayerProfile {
    pub authority: Pubkey,
    #[max_len(2)]
    pub country: String, // ISO 3166-1 alpha-2
    pub wins: u32,
    pub losses: u32,
    pub draws: u32,
    pub games_played: u32,
    pub elo_rating: f64,  // Changed from u32 to f64 for Glicko-2
    pub rd: f64,          // Rating deviation
    pub volatility: f64,  // Rating volatility
    pub last_played: i64, // For inactivity handling
    pub win_streak: u32,
    pub best_streak: u32,
    pub tournament_wins: u32,
    pub ranked_games: u32,
    pub total_wagered: u64,
    pub total_won: u64,
    pub created_at: i64,
    pub last_game_at: i64,
    /// Unix timestamp of date of birth. Used to enforce 18+ age gate on-chain.
    pub date_of_birth: i64,
    pub is_verified: bool,
    pub annual_wins_gbp: u64, // UK: annual wins in GBP
    pub annual_wins_brl: u64, // Brazil: annual wins in BRL
    pub annual_wins_cad: u64, // Canada: annual wins in CAD
    pub annual_wins_eur: u64, // Germany: annual wins in EUR
    #[max_len(20)]
    pub username: String,
    pub username_set: bool,

    // ── External Lichess platform linkage ──
    #[max_len(30)]
    pub lichess_username: String,
    pub lichess_verified: bool,
    pub lichess_blitz: u32, // stored in centiscale (rating * 100)
    pub lichess_rapid: u32,
    pub lichess_bullet: u32,
    pub lichess_last_sync: i64, // Unix timestamp of last successful sync

    pub external_elo_source: u8, // 0=none, 1=lichess
    pub seeded_from_external: bool,
}

// Old simplified Glicko-2 implementation removed - will be replaced with on-chain Glicko-2 calculation
