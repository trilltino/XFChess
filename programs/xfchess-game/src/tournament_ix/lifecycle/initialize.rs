//! Instruction to bootstrap a new bracket-based tournament.
//! Supports 2, 4, 8, 16, 32, 64, 128, 256 player single-elimination and Swiss tournaments.
//! Player data is sharded across up to 4 TournamentPlayersShard PDAs (64 players each).

use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, Token, TokenAccount};

/// Valid player counts (must be power of 2 for single-elimination, any for Swiss).
const VALID_PLAYER_COUNTS: [u16; 8] = [2, 4, 8, 16, 32, 64, 128, 256];

#[derive(Accounts)]
#[instruction(
    tournament_id: u64,
    max_players: u16,
    tournament_type: TournamentType,
    elo_min: u32,
    elo_max: u32,
    min_players: u16,
    prize_shares: [u16; 10],
    platform_fee: u64,
    winner_takes_all: bool,
    host_treasury: Pubkey,
    usdc_mint: Option<Pubkey>,
    base_time_seconds: u64,
    increment_seconds: u16
)]
pub struct InitializeTournament<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + Tournament::space_for(max_players),
        seeds = [TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament: Account<'info, Tournament>,
    /// CHECK: USDC prize escrow PDA — the authority of the token account.
    #[account(
        seeds = [TOURNAMENT_USDC_PRIZE_SEED, &tournament_id.to_le_bytes()],
        bump
    )]
    pub usdc_prize_escrow_authority: UncheckedAccount<'info>,
    /// USDC prize escrow token account (initialized if usdc_mint is Some).
    #[account(
        init_if_needed,
        payer = authority,
        associated_token::mint = usdc_mint,
        associated_token::authority = usdc_prize_escrow_authority,
    )]
    pub usdc_prize_escrow: Option<Account<'info, TokenAccount>>,
    /// The USDC mint account (optional).
    pub usdc_mint: Option<Account<'info, token::Mint>>,
    #[account(
        mut,
        constraint = authority.key() == vps_authority::ID @ GameErrorCode::UnauthorizedAccess
    )]
    pub authority: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<InitializeTournament>,
    tournament_id: u64,
    name: String,
    entry_fee: u64,
    max_players: u16,
    tournament_type: TournamentType,
    elo_min: u32,
    elo_max: u32,
    min_players: u16,
    prize_shares: [u16; 10],
    platform_fee: u64,
    winner_takes_all: bool,
    host_treasury: Pubkey,
    usdc_mint: Option<Pubkey>,
    base_time_seconds: u64,
    increment_seconds: u16,
) -> Result<()> {
    require!(name.len() <= 64, GameErrorCode::InvalidGameStatus);
    require!(
        VALID_PLAYER_COUNTS.contains(&max_players),
        GameErrorCode::InvalidGameStatus
    );

    // Validate ELO range
    require!(elo_min <= elo_max, GameErrorCode::InvalidGameStatus);
    require!(min_players <= max_players, GameErrorCode::InvalidGameStatus);

    // Validate prize shares sum to <= 10000
    let total_shares: u16 = prize_shares.iter().sum();
    require!(total_shares <= 10000, GameErrorCode::InvalidGameStatus);

    // Validate USDC mint is provided if usdc_prize_escrow is being initialized
    if ctx.accounts.usdc_prize_escrow.is_some() {
        require!(usdc_mint.is_some(), GameErrorCode::InvalidGameStatus);
    }

    // Use default prize shares if winner_takes_all, otherwise use provided shares
    let final_prize_shares = if winner_takes_all {
        crate::state::tournament::get_default_prize_shares(max_players, true)
    } else {
        prize_shares
    };

    msg!(
        "Tournament space: {}",
        8 + Tournament::space_for(max_players)
    );
    msg!(
        "TournamentPlayersShard space per shard: {}",
        8 + TournamentPlayersShard::space_for()
    );

    let t = &mut ctx.accounts.tournament;
    let total_matches = max_players - 1;

    t.tournament_id = tournament_id;
    t.authority = ctx.accounts.authority.key();
    t.name = name;
    t.entry_fee = entry_fee;
    t.platform_fee = platform_fee;
    t.prize_pool = 0;
    t.max_players = max_players;
    t.num_registered_players = 0;
    t.status = TournamentStatus::Registration;
    t.tournament_type = tournament_type.clone();
    t.current_round = 0;
    t.total_rounds = match tournament_type {
        TournamentType::Swiss { rounds } => rounds,
        TournamentType::SingleElimination => 0,
    };
    t.total_matches = total_matches;
    t.final_match_index = total_matches - 1;
    t.elo_min = elo_min;
    t.elo_max = elo_max;
    t.min_players = min_players;
    t.winner = None;
    t.second_place = None;
    t.third_place = None;
    t.fourth_place = None;
    t.fifth_place = None;
    t.sixth_place = None;
    t.seventh_place = None;
    t.eighth_place = None;
    t.ninth_place = None;
    t.tenth_place = None;
    t.prize_shares = final_prize_shares;
    t.created_at = Clock::get()?.unix_timestamp;
    t.started_at = None;
    t.completed_at = None;
    t.bump = ctx.bumps.tournament;
    t.fee_payer = ctx.accounts.authority.key();
    t.fees_advanced = 0;
    t.platform_fee_pool = 0;
    // USDC prize pool fields
    t.usdc_prize_mint = usdc_mint;
    t.usdc_prize_pool = 0;
    t.usdc_prize_funded = false;
    t.host_treasury = host_treasury;
    t.base_time_seconds = base_time_seconds;
    t.increment_seconds = increment_seconds;

    Ok(())
}
