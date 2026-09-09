use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct Tournament {
    pub tournament_id: u64,
    pub authority: Pubkey,
    #[max_len(64)]
    pub name: String,
    pub entry_fee: u64,
    pub platform_fee: u64,
    pub prize_pool: u64,
    pub max_players: u16,
    pub player_count: u16,
    pub num_registered_players: u16,
    pub status: TournamentStatus,
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
    pub fees_advanced: u64, // Accumulator for operational fees paid by relayer
    pub fee_payer: Pubkey,  // Relayer wallet that paid; reimbursed at claim
    pub tournament_type: TournamentType,
    pub current_round: u8,
    pub total_rounds: u8,
    pub total_matches: u16,
    pub final_match_index: u16,
    pub elo_min: u32,
    pub elo_max: u32,
    pub min_players: u16,
    pub winner: Option<Pubkey>,
    pub second_place: Option<Pubkey>,
    pub third_place: Option<Pubkey>,
    pub fourth_place: Option<Pubkey>,
    pub fifth_place: Option<Pubkey>,
    pub sixth_place: Option<Pubkey>,
    pub seventh_place: Option<Pubkey>,
    pub eighth_place: Option<Pubkey>,
    pub ninth_place: Option<Pubkey>,
    pub tenth_place: Option<Pubkey>,
    pub prize_shares: [u16; 10],
    pub created_at: i64,
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
    pub bump: u8,
    // USDC prize pool fields (new)
    pub prizes_claimed: u16,
    pub platform_fee_pool: u64,
    pub usdc_prize_mint: Option<Pubkey>,
    pub usdc_prize_pool: u64,
    pub usdc_prize_funded: bool,
    pub host_treasury: Pubkey,
    pub prize_token_mint: Option<Pubkey>,
    pub payout_type: PayoutType,
    pub vesting_params: Option<VestingParams>,
    pub base_time_seconds: u64,
    pub increment_seconds: u16,
    pub winner_takes_all: bool,
    pub round_boards_reported: [u8; 16],
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace, Debug)]
pub enum PayoutType {
    LumpSum,
    StreamingLinear,
    StreamingCliff,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace, Debug)]
pub struct VestingParams {
    pub start_time: i64,
    pub duration_seconds: i64,
    pub cliff_seconds: Option<i64>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq, InitSpace)]
pub enum TournamentType {
    Swiss { rounds: u8 },
    SingleElimination,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace, Debug)]
pub enum TournamentStatus {
    Registration,
    Active,
    Completed,
    Closed,
    Cancelled,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, InitSpace, Debug)]
pub struct SwissStanding {
    pub player: Pubkey,
    pub score: u8,         // Points: 2 for win, 1 for draw, 0 for loss
    pub buchholz: u16,     // Sum of opponents' scores
    pub sonneborn: u16,    // Sum of defeated opponents' scores + 0.5*draws
    pub color_balance: i8, // Whites - blacks (should balance to 0)
}

impl Tournament {
    pub fn space_for(_max_players: u16) -> usize {
        Self::INIT_SPACE
    }
}

#[account]
pub struct TournamentPlayersShard {
    pub tournament_id: u64,
    pub shard_id: u8,
    pub players: Vec<Pubkey>,
    pub player_elos: Vec<u32>,
    pub swiss_standings: Vec<SwissStanding>,
}

impl TournamentPlayersShard {
    pub const SHARD_CAPACITY: u16 = 64;
    pub fn space_for() -> usize {
        8 + 1
            + 4
            + (Self::SHARD_CAPACITY as usize) * 32
            + 4
            + (Self::SHARD_CAPACITY as usize) * 4
            + 4
            + (Self::SHARD_CAPACITY as usize) * 38
    }
}

pub fn get_default_prize_shares(max_players: u16, winner_takes_all: bool) -> [u16; 10] {
    if winner_takes_all {
        return [10000, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    }

    match max_players {
        0..=2 => {
            // Head-to-head: only 1st and 2nd exist — 70/30%
            [7000, 3000, 0, 0, 0, 0, 0, 0, 0, 0]
        }
        3..=64 => {
            // Top 3: 60/30/10%
            [6000, 3000, 1000, 0, 0, 0, 0, 0, 0, 0]
        }
        128 => {
            // Top 5: 50/25/15/5/5% (4th and 5th equal)
            [5000, 2500, 1500, 500, 500, 0, 0, 0, 0, 0]
        }
        256 => {
            // Top 10: 40/20/12/8/6/4/3/2/2/3% (top prizes attractive, 7-10 get smaller)
            [4000, 2000, 1200, 800, 600, 400, 300, 200, 200, 300]
        }
        _ => {
            // Default to 64 and below distribution
            [6000, 3000, 1000, 0, 0, 0, 0, 0, 0, 0]
        }
    }
}
