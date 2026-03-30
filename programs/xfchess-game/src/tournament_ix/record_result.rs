use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(tournament_id: u64, match_index: u8)]
pub struct RecordMatchResult<'info> {
    #[account(
        mut,
        seeds = [TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        bump = tournament.bump,
        constraint = tournament.authority == authority.key() @ GameErrorCode::NotTournamentAuthority
    )]
    pub tournament: Account<'info, Tournament>,
    #[account(
        mut,
        seeds = [TOURNAMENT_MATCH_SEED, &tournament_id.to_le_bytes(), &[match_index]],
        bump = tournament_match.bump
    )]
    pub tournament_match: Account<'info, TournamentMatch>,
    /// CHECK: The game PDA for this match — read-only for result verification.
    pub game: UncheckedAccount<'info>,
    #[account(mut)]
    pub authority: Signer<'info>,
}

pub fn handler(
    ctx: Context<RecordMatchResult>,
    tournament_id: u64,
    match_index: u8,
    winner: Pubkey,
) -> Result<()> {
    let tm = &mut ctx.accounts.tournament_match;

    require!(
        tm.status == MatchStatus::Active || tm.status == MatchStatus::Pending,
        GameErrorCode::InvalidMatchStatus
    );

    tm.winner = Some(winner);
    tm.status = MatchStatus::Completed;
    tm.completed_at = Some(Clock::get()?.unix_timestamp);

    let tournament = &mut ctx.accounts.tournament;

    if match_index < 2 {
        // Semi-final completed — check if both SFs done to set up final
        // Backend calls advance_final after both SFs are recorded
        msg!(
            "Tournament {} SF{} completed. Winner: {}",
            tournament_id,
            match_index + 1,
            winner
        );
    } else {
        // Final completed
        tournament.winner = Some(winner);
        tournament.status = TournamentStatus::Completed;
        tournament.completed_at = Some(Clock::get()?.unix_timestamp);
        msg!(
            "Tournament {} FINAL completed. Champion: {}",
            tournament_id,
            winner
        );
    }

    Ok(())
}

/// Separate instruction to set up the final match after both semi-finals complete.
#[derive(Accounts)]
#[instruction(tournament_id: u64)]
pub struct AdvanceFinal<'info> {
    #[account(
        mut,
        seeds = [TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        bump = tournament.bump,
        constraint = tournament.authority == authority.key() @ GameErrorCode::NotTournamentAuthority
    )]
    pub tournament: Account<'info, Tournament>,
    #[account(
        seeds = [TOURNAMENT_MATCH_SEED, &tournament_id.to_le_bytes(), &[0u8]],
        bump = sf1.bump
    )]
    pub sf1: Account<'info, TournamentMatch>,
    #[account(
        seeds = [TOURNAMENT_MATCH_SEED, &tournament_id.to_le_bytes(), &[1u8]],
        bump = sf2.bump
    )]
    pub sf2: Account<'info, TournamentMatch>,
    #[account(
        mut,
        seeds = [TOURNAMENT_MATCH_SEED, &tournament_id.to_le_bytes(), &[2u8]],
        bump = final_match.bump
    )]
    pub final_match: Account<'info, TournamentMatch>,
    #[account(mut)]
    pub authority: Signer<'info>,
}

pub fn handler_advance_final(ctx: Context<AdvanceFinal>, _tournament_id: u64) -> Result<()> {
    require!(
        ctx.accounts.sf1.status == MatchStatus::Completed,
        GameErrorCode::InvalidMatchStatus
    );
    require!(
        ctx.accounts.sf2.status == MatchStatus::Completed,
        GameErrorCode::InvalidMatchStatus
    );

    let sf1_winner = ctx.accounts.sf1.winner.ok_or(GameErrorCode::InvalidMatchStatus)?;
    let sf2_winner = ctx.accounts.sf2.winner.ok_or(GameErrorCode::InvalidMatchStatus)?;

    let fin = &mut ctx.accounts.final_match;
    fin.player_white = Some(sf1_winner);
    fin.player_black = Some(sf2_winner);
    fin.status = MatchStatus::Pending;

    ctx.accounts.tournament.current_round = 1;

    msg!(
        "Final match set: {} (white) vs {} (black)",
        sf1_winner,
        sf2_winner
    );
    Ok(())
}
