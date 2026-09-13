use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace, Default)]
pub struct PlayerProfile {
    pub authority: Pubkey,
    #[max_len(2)]
    pub country: String,
    pub wins: u32,
    pub losses: u32,
    pub draws: u32,
    pub games_played: u32,
    pub elo_rating: f64,
    pub rd: f64,
    pub volatility: f64,
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
    #[max_len(20)]
    pub username: String,
    pub username_set: bool,

    #[max_len(30)]
    pub lichess_username: String,
    pub lichess_verified: bool,
    pub lichess_blitz: u32,
    pub lichess_rapid: u32,
    pub lichess_bullet: u32,
    pub lichess_last_sync: i64,

    pub external_elo_source: u8,
    pub seeded_from_external: bool,

    pub elo_bullet: f64,
    pub elo_blitz: f64,
    pub elo_rapid: f64,
}
