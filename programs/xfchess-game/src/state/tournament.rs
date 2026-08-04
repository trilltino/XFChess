//! State structure defining tournament meta-info, prize pools, and progression.
//! Supports 2, 4, 8, 16, 32, 64, 128, 256 player single-elimination and Swiss tournaments.

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
    /// Maximum players (must be power of 2 for single-elimination: 2, 4, 8, 16, 32, 64, 128, 256).
    pub max_players: u16,
    /// Current number of registered players.
    pub player_count: u16,
    /// Current number of registered players.
    pub num_registered_players: u16,
    pub status: TournamentStatus,
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
    pub fees_advanced: u64, // Accumulator for operational fees paid by relayer
    pub fee_payer: Pubkey,  // Relayer wallet that paid; reimbursed at claim
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
    pub created_at: i64,
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
    pub bump: u8,
    // USDC prize pool fields (new)
    /// Bitflags for claimed prizes (bit 0 = winner, bit 1 = second_place, etc.)
    pub prizes_claimed: u16,
    /// Accumulated platform fee pool (from £0.50 cuts) to cover transaction fees and rent
    pub platform_fee_pool: u64,
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
    /// Total clock per player in seconds for each match (0 = no time limit).
    pub base_time_seconds: u64,
    /// Fischer increment added after each move in seconds.
    pub increment_seconds: u16,
    /// Winner takes all flag.
    pub winner_takes_all: bool,
    /// Bitmap: bit `board` is set once `record_swiss_result` has recorded
    /// that board's result for `current_round`. Reset to all-zero whenever
    /// `advance_round` bumps `current_round`. Sized for the worst case (a
    /// 256-player Swiss tournament has up to 128 boards per round).
    /// Lets `advance_round` verify a round is actually complete purely from
    /// on-chain state instead of trusting an off-chain caller — the whole
    /// point being that round progression doesn't need the backend alive.
    pub round_boards_reported: [u8; 16],
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

/// Swiss tournament standing for a single player.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, InitSpace, Debug)]
pub struct SwissStanding {
    pub player: Pubkey,
    pub score: u8,         // Points: 2 for win, 1 for draw, 0 for loss
    pub buchholz: u16,     // Sum of opponents' scores
    pub sonneborn: u16,    // Sum of defeated opponents' scores + 0.5*draws
    pub color_balance: i8, // Whites - blacks (should balance to 0)
}

impl Tournament {
    /// Bytes needed for tournament metadata only (player data lives in TournamentPlayersShard PDAs).
    pub fn space_for(_max_players: u16) -> usize {
        Self::INIT_SPACE
    }
}

/// Separate PDA account holding player lists and Swiss standings for a single shard.
/// Keeps Tournament metadata small so initialization fits CPI realloc limit.
/// Space is calculated manually to avoid InitSpace over-allocation.
/// Each shard holds up to 64 players; 4 shards support 256-player tournaments.
#[account]
pub struct TournamentPlayersShard {
    pub tournament_id: u64,
    pub shard_id: u8,
    pub players: Vec<Pubkey>,
    pub player_elos: Vec<u32>,
    pub swiss_standings: Vec<SwissStanding>,
}

impl TournamentPlayersShard {
    /// Bytes needed for a single TournamentPlayersShard PDA with 64-player capacity.
    /// Anchor `init` adds 8 for discriminator automatically, so we exclude it here.
    /// Layout: 8 (tournament_id) + 1 (shard_id) + 4 + 64*32 (players) + 4 + 64*4 (elos) + 4 + 64*38 (standings)
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

/// Returns competitive default prize distribution based on tournament size.
/// If winner_takes_all is true, returns [10000, 0, 0, 0, 0, 0, 0, 0, 0, 0].
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
