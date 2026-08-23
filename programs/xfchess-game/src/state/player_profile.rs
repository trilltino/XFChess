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
    /// Classical-time-control rating (centiscale, ×100) — also the bucket used
    /// for `Unlimited`/no-clock games. Updated by K=32 Elo in
    /// `lifecycle::settlement`. See `elo/README.md`. Per-mode siblings:
    /// `elo_bullet`, `elo_blitz`, `elo_rapid` (appended at the end of this
    /// struct, below `seeded_from_external`, so existing byte offsets for
    /// every field above them are unaffected).
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
    /// Unused — `link_external_elo` no longer seeds `elo_rating` from a
    /// linked account (removed: a one-time Lichess-derived seed was
    /// confusing and gave a wagering-relevant rating a moment of external
    /// influence). Kept as a dead field rather than removed, since Anchor
    /// account layout changes require appending, and shrinking would shift
    /// every field below it. Existing profiles seeded before the removal
    /// keep whatever value they already had; nothing sets it going forward.
    pub seeded_from_external: bool,

    // ── Per-time-control ratings (centiscale) ──
    // Appended at the end so every existing field's byte offset — and every
    // hand-parsed offset reader (e.g. `backend/src/signing/elo_cache.rs`) —
    // is unaffected. `elo_rating` above doubles as the Classical/Unlimited
    // bucket; these three cover the rest of `TimeCategory`
    // (`src/game/time_control.rs`), folding `UltraBullet` into `Bullet`.
    // Each starts at `0.0` and is lazily seeded to `INITIAL_ELO_CENTISCALE`
    // the first time a game in that bucket settles — see
    // `lifecycle::settlement::rating_field_for_time_control`.
    pub elo_bullet: f64,
    pub elo_blitz: f64,
    pub elo_rapid: f64,
}
