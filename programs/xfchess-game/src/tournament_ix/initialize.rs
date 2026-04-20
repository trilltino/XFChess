//! Instruction to bootstrap a new bracket-based tournament.
//! Supports 8, 16, 32, 64, 128, 256 player single-elimination and Swiss tournaments.

use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, Token, TokenAccount};

/// Valid player counts (must be power of 2 for single-elimination, any for Swiss).
const VALID_PLAYER_COUNTS: [u16; 6] = [8, 16, 32, 64, 128, 256];

#[derive(Accounts)]
#[instruction(
    tournament_id: u64,
    max_players: u16,
    tournament_type: TournamentType,
    elo_min: u32,
    elo_max: u32,
    min_players: u16,
    prize_shares: [u16; 8],
    host_treasury: Pubkey,
    usdc_mint: Option<Pubkey>
)]
pub struct InitializeTournament<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + Tournament::INIT_SPACE,
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
    prize_shares: [u16; 8],
    host_treasury: Pubkey,
    usdc_mint: Option<Pubkey>,
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

    let t = &mut ctx.accounts.tournament;
    let total_matches = max_players - 1;

    t.tournament_id = tournament_id;
    t.authority = ctx.accounts.authority.key();
    t.name = name;
    t.entry_fee = entry_fee;
    t.prize_pool = 0;
    t.max_players = max_players;
    t.registered_count = 0;
    t.status = TournamentStatus::Registration;
    t.tournament_type = tournament_type;
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
    t.prize_shares = prize_shares;
    t.players = Vec::with_capacity(max_players as usize);
    t.player_elos = Vec::with_capacity(max_players as usize);
    t.swiss_standings = Vec::with_capacity(max_players as usize);
    t.created_at = Clock::get()?.unix_timestamp;
    t.started_at = None;
    t.completed_at = None;
    t.bump = ctx.bumps.tournament;
    // USDC prize pool fields
    t.usdc_prize_mint = usdc_mint;
    t.usdc_prize_pool = 0;
    t.usdc_prize_funded = false;
    t.host_treasury = host_treasury;

    msg!(
        "Tournament {} '{}' created. Type: {:?}, Players: {}, Entry fee: {} lamports, Host treasury: {}",
        tournament_id,
        t.name,
        tournament_type,
        max_players,
        entry_fee,
        host_treasury
    );
    
    if usdc_mint.is_some() {
        msg!("USDC prize pool enabled. Mint: {:?}", usdc_mint);
    }
    
    Ok(())
}
