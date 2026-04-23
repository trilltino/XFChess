//! State structure defining tournament meta-info, prize pools, and progression.
//! Supports 8, 16, 32, 64, 128, 256 player single-elimination and Swiss tournaments.

use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct Tournament {
    pub tournament_id: u64,
    pub authority: Pubkey,
    #[max_len(64)]
    pub name: String,
    pub entry_fee: u64,
    pub prize_pool: u64,
    /// Maximum players (must be power of 2 for single-elimination: 8, 16, 32, 64, 128, 256).
    pub max_players: u16,
    /// Current number of registered players.
    pub registered_count: u16,
    pub status: TournamentStatus,
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
    pub fees_advanced: u64, // Accumulator for operational fees paid by relayer
    pub fee_payer: Pubkey, // Relayer wallet that paid; reimbursed at claim
    pub tournament_type: TournamentType,
    pub current_round: u8,
    /// Total rounds (for Swiss tournaments only).
    pub total_rounds: u8,
    /// Total number of matches in the tournament (max_players - 1 for single-elimination).
    pub total_matches: u16,
    /// Index of the final match (always total_matches - 1).
    pub final_match_index: u16,
    /// ELO bracket filtering.
    pub elo_min: u32,
    pub elo_max: u32,
    /// Minimum players required to start tournament.
    pub min_players: u16,
    /// Tournament winner (set when final completes).
    pub winner: Option<Pubkey>,
    /// Second place (final loser).
    pub second_place: Option<Pubkey>,
    /// Third place (semifinal loser with higher ELO or first semifinal).
    pub third_place: Option<Pubkey>,
    /// Fourth place (other semifinal loser).
    pub fourth_place: Option<Pubkey>,
    /// Fifth place
    pub fifth_place: Option<Pubkey>,
    /// Sixth place
    pub sixth_place: Option<Pubkey>,
    /// Seventh place
    pub seventh_place: Option<Pubkey>,
    /// Eighth place
    pub eighth_place: Option<Pubkey>,
    /// Ninth place
    pub ninth_place: Option<Pubkey>,
    /// Tenth place
    pub tenth_place: Option<Pubkey>,
    /// Prize distribution for top 10: [1st%, 2nd%, 3rd%, 4th%, 5th%, 6th%, 7th%, 8th%, 9th%, 10th%] in basis points.
    /// 64 and below (top 3): [6000, 3000, 1000, 0, 0, 0, 0, 0, 0, 0] = 60/30/10%
    /// 128 players (top 5): [5000, 2500, 1500, 500, 500, 0, 0, 0, 0, 0] = 50/25/15/5/5%
    /// 256 players (top 10): [4000, 2000, 1200, 800, 600, 400, 300, 200, 200, 300] = 40/20/12/8/6/4/3/2/2/3%
    /// Winner-takes-all: [10000, 0, 0, 0, 0, 0, 0, 0, 0, 0]
    pub prize_shares: [u16; 10],
    /// Registered players (up to 256).
    #[max_len(256)]
    pub players: Vec<Pubkey>,
    /// ELO ratings mirrored at registration time for bracket seeding.
    #[max_len(256)]
    pub player_elos: Vec<u32>,
    /// Swiss tournament standings (only used for Swiss tournaments).
    #[max_len(256)]
    pub swiss_standings: Vec<SwissStanding>,
    pub created_at: i64,
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
    pub bump: u8,
    // USDC prize pool fields (new)
    /// USDC mint address (None = SOL-only tournament).
    pub usdc_prize_mint: Option<Pubkey>,
    /// Total USDC locked in prize escrow (in USDC base units, 6 decimals).
    pub usdc_prize_pool: u64,
    /// True once operator has deposited USDC prize pool.
    pub usdc_prize_funded: bool,
    /// Operator treasury wallet — entry fees go here directly.
    pub host_treasury: Pubkey,
    /// Prize token mint (None = wrapped SOL, overrides usdc_prize_mint for generic SPL)
    pub prize_token_mint: Option<Pubkey>,
    /// Prize distribution method
    pub payout_type: PayoutType,
    /// Streaming vesting parameters (if applicable)
    pub vesting_params: Option<VestingParams>,
}

/// Prize distribution type
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace, Debug)]
pub enum PayoutType {
    /// Immediate full payout
    LumpSum,
    /// Linear vesting over N days
    StreamingLinear,
    /// Cliff vesting (e.g., 50% at 30 days, 50% at 60 days)
    StreamingCliff,
}

/// Vesting parameters for streaming payouts
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace, Debug)]
pub struct VestingParams {
    /// When vesting starts (Unix timestamp)
    pub start_time: i64,
    /// Total vesting duration in seconds
    pub duration_seconds: i64,
    /// Optional cliff period in seconds
    pub cliff_seconds: Option<i64>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace, Debug)]
pub enum TournamentStatus {
    Registration,
    Active,
    Completed,
    Cancelled,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace, Debug)]
pub enum TournamentType {
    SingleElimination,
    Swiss { rounds: u8 },
}

/// Swiss tournament standing for a single player.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, InitSpace, Debug)]
pub struct SwissStanding {
    pub player: Pubkey,
    pub score: u8,  // Points: 2 for win, 1 for draw, 0 for loss
    pub buchholz: u16,  // Sum of opponents' scores
    pub sonneborn: u16,  // Sum of defeated opponents' scores + 0.5*draws
    pub color_balance: i8,  // Whites - blacks (should balance to 0)
}

/// Returns competitive default prize distribution based on tournament size.
/// If winner_takes_all is true, returns [10000, 0, 0, 0, 0, 0, 0, 0, 0, 0].
pub fn get_default_prize_shares(max_players: u16, winner_takes_all: bool) -> [u16; 10] {
    if winner_takes_all {
        return [10000, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    }

    match max_players {
        0..=64 => {
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
