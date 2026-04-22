//! Instruction to initialize a single tournament match.
//! Called by the backend to set up the bracket after tournament starts.

use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(tournament_id: u64, match_index: u16)]
pub struct InitializeMatch<'info> {
    #[account(
        mut,
        seeds = [TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        bump = tournament.bump,
        constraint = tournament.authority == authority.key() @ GameErrorCode::NotTournamentAuthority
    )]
    pub tournament: Account<'info, Tournament>,
    #[account(
        init,
        payer = authority,
        space = 8 + TournamentMatch::INIT_SPACE,
        seeds = [TOURNAMENT_MATCH_SEED, &tournament_id.to_le_bytes(), &match_index.to_le_bytes()],
        bump
    )]
    pub tournament_match: Account<'info, TournamentMatch>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<InitializeMatch>,
    tournament_id: u64,
    match_index: u16,
    round: u8,
    player_white: Option<Pubkey>,
    player_black: Option<Pubkey>,
    next_match_for_winner: Option<u16>,
    next_match_slot: u8,
) -> Result<()> {
    require!(
        ctx.accounts.tournament.status == TournamentStatus::Active,
        GameErrorCode::TournamentNotInRegistration
    );
    require!(
        match_index < ctx.accounts.tournament.total_matches,
        GameErrorCode::InvalidMatchStatus
    );

    let tm = &mut ctx.accounts.tournament_match;
    tm.tournament_id = tournament_id;
    tm.match_index = match_index;
    tm.round = round;
    tm.player_white = player_white;
    tm.player_black = player_black;
    tm.winner = None;
    tm.game_pda = None;
    tm.game_id = None;
    tm.status = MatchStatus::Pending;
    tm.next_match_for_winner = next_match_for_winner;
    tm.next_match_slot = next_match_slot;
    tm.started_at = None;
    tm.completed_at = None;
    tm.bump = ctx.bumps.tournament_match;

    msg!(
        "Match {} initialized for tournament {} (round {})",
        match_index,
        tournament_id,
        round
    );
    Ok(())
}
