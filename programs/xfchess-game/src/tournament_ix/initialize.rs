//! Instruction to bootstrap a new bracket-based tournament.
//! Supports 8, 16, 32, 64, 128 player single-elimination tournaments.

use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;

/// Valid player counts (must be power of 2 for single-elimination).
const VALID_PLAYER_COUNTS: [u16; 5] = [8, 16, 32, 64, 128];

#[derive(Accounts)]
#[instruction(tournament_id: u64, max_players: u16, prize_shares: [u16; 4])]
pub struct InitializeTournament<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + Tournament::INIT_SPACE,
        seeds = [TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament: Account<'info, Tournament>,
    /// CHECK: PDA escrow holding entry fees until payout or refund.
    #[account(
        mut,
        seeds = [TOURNAMENT_ESCROW_SEED, &tournament_id.to_le_bytes()],
        bump
    )]
    pub escrow_pda: UncheckedAccount<'info>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<InitializeTournament>,
    tournament_id: u64,
    name: String,
    entry_fee: u64,
    max_players: u16,
    prize_shares: [u16; 4],
) -> Result<()> {
    require!(name.len() <= 64, GameErrorCode::InvalidGameStatus);
    require!(
        VALID_PLAYER_COUNTS.contains(&max_players),
        GameErrorCode::InvalidGameStatus
    );

    // Validate prize shares sum to 10000 (100%)
    let total_shares: u16 = prize_shares.iter().sum();
    require!(total_shares == 10000, GameErrorCode::InvalidGameStatus);

    let t = &mut ctx.accounts.tournament;
    let total_matches = max_players - 1;

    t.tournament_id = tournament_id;
    t.authority = ctx.accounts.authority.key();
    t.name = name;
    t.entry_fee = entry_fee;
    t.prize_pool = 0;
    t.max_players = max_players;
    t.registered_count = 0;
    t.total_matches = total_matches;
    t.final_match_index = total_matches - 1;
    t.status = TournamentStatus::Registration;
    t.current_round = 0;
    t.winner = None;
    t.second_place = None;
    t.third_place = None;
    t.fourth_place = None;
    t.prize_shares = prize_shares;
    t.players = Vec::with_capacity(max_players as usize);
    t.player_elos = Vec::with_capacity(max_players as usize);
    t.created_at = Clock::get()?.unix_timestamp;
    t.started_at = None;
    t.completed_at = None;
    t.bump = ctx.bumps.tournament;

    msg!(
        "Tournament {} '{}' created. Players: {}, Entry fee: {} lamports, Prize shares: {:?}",
        tournament_id,
        t.name,
        max_players,
        entry_fee,
        prize_shares
    );
    Ok(())
}
