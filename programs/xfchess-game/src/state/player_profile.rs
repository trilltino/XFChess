//! Account structure encompassing a player's long-term ranking and stats.

use anchor_lang::prelude::*;

/// A player's long-term identity, ranking, and stats. One PDA per wallet
/// (seed: `[PROFILE_SEED, authority]`). Created/updated via `account_ix`;
/// gameplay stats and `elo_rating` are updated by `lifecycle::settlement`.
#[account]
#[derive(InitSpace, Default)]
pub struct PlayerProfile {
    pub authority: Pubkey,
    /// ISO 3166-1 alpha-2 country code, used for jurisdiction/compliance checks.
    #[max_len(2)]
    pub country: String,
    pub wins: u32,
    pub losses: u32,
    pub draws: u32,
    pub games_played: u32,
    /// Centiscale rating (×100), updated by K=32 Elo in `lifecycle::settlement`. See `elo/README.md`.
    pub elo_rating: f64,
    /// Unused — reserved from an abandoned Glicko-2 rating attempt; never read or written.
    pub rd: f64,
    /// Unused — reserved from an abandoned Glicko-2 rating attempt; never read or written.
    pub volatility: f64,
    /// Unix timestamp of the last completed game, for inactivity handling.
    pub last_played: i64,
    pub win_streak: u32,
    pub best_streak: u32,
    pub tournament_wins: u32,
    /// Ranked (non-free) games played; drives the ELO-linking fee split in settlement.
    pub ranked_games: u32,
    pub total_wagered: u64,
    pub total_won: u64,
    pub created_at: i64,
    pub last_game_at: i64,
    /// Unix timestamp of date of birth. Used to enforce 18+ age gate on-chain.
    pub date_of_birth: i64,
    /// KYC-verified via `account_ix::profile::verify_handler`.
    pub is_verified: bool,
    /// UK: annual wins in GBP, for jurisdictional wager-limit tracking.
    pub annual_wins_gbp: u64,
    /// Brazil: annual wins in BRL, for jurisdictional wager-limit tracking.
    pub annual_wins_brl: u64,
    /// Canada: annual wins in CAD, for jurisdictional wager-limit tracking.
    pub annual_wins_cad: u64,
    /// Germany: annual wins in EUR, for jurisdictional wager-limit tracking.
    pub annual_wins_eur: u64,
    #[max_len(20)]
    pub username: String,
    pub username_set: bool,

    // ── External Lichess platform linkage ──
    #[max_len(30)]
    pub lichess_username: String,
    pub lichess_verified: bool,
    /// Lichess blitz rating in centiscale (rating × 100).
    pub lichess_blitz: u32,
    /// Lichess rapid rating in centiscale (rating × 100).
    pub lichess_rapid: u32,
    /// Lichess bullet rating in centiscale (rating × 100).
    pub lichess_bullet: u32,
    /// Unix timestamp of the last successful Lichess rating sync.
    pub lichess_last_sync: i64,

    /// Source of the external rating link: 0 = none, 1 = Lichess.
    pub external_elo_source: u8,
    /// True once `elo_rating` has been seeded from an external link — gates
    /// the one-time seed logic in `account_ix::link_external_elo::handler`.
    pub seeded_from_external: bool,
}
