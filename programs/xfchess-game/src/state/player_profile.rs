use anchor_lang::prelude::*;

/// Player record. Every fixed-size field comes first, then the three variable-length strings,
/// so a numeric field has the same byte offset in every account (backend decoders rely on
/// this; `services/backend/src/signing/elo_cache.rs` has a layout test).
///
/// Ratings are fixed-point `u32` in hundredths of a rating point (1500.00 is 150_000); there
/// are no floats on-chain.
#[account]
#[derive(InitSpace, Default)]
pub struct PlayerProfile {
    pub authority: Pubkey,
    pub wins: u32,
    pub losses: u32,
    pub draws: u32,
    pub games_played: u32,

    /// Classical rating (and the fallback bucket for games with no clock).
    pub elo_rating: u32,
    pub elo_bullet: u32,
    pub elo_blitz: u32,
    pub elo_rapid: u32,
    /// Rated games played per time control; drives the provisional K. Saturates at 255.
    pub games_classical: u8,
    pub games_bullet: u8,
    pub games_blitz: u8,
    pub games_rapid: u8,
    /// First 8 bytes of the last rated opponent, and how many rated games in a row have been
    /// played against them (see `elo::repeat`).
    pub last_rated_opponent: [u8; 8],
    pub repeat_streak: u8,

    pub last_played: i64,
    pub win_streak: u32,
    pub best_streak: u32,
    pub tournament_wins: u32,
    pub ranked_games: u32,
    pub total_wagered: u64,
    pub total_won: u64,
    pub created_at: i64,
    pub last_game_at: i64,
    pub date_of_birth: i64,
    pub is_verified: bool,
    pub annual_wins_gbp: u64,
    pub annual_wins_brl: u64,
    pub annual_wins_cad: u64,
    pub annual_wins_eur: u64,
    pub username_set: bool,

    pub lichess_verified: bool,
    pub lichess_blitz: u32,
    pub lichess_rapid: u32,
    pub lichess_bullet: u32,
    pub lichess_last_sync: i64,
    pub external_elo_source: u8,
    /// Set once, when a linked external rating first seeds the starting ratings.
    pub seeded_from_external: bool,

    #[max_len(2)]
    pub country: String,
    #[max_len(20)]
    pub username: String,
    #[max_len(30)]
    pub lichess_username: String,
}
