//! Instruction resolving an individual tournament game to advance players.
//! Supports dynamic single-elimination brackets of any size.

use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use crate::tournament_ix::matches::guards;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(tournament_id: u64, match_index: u16)]
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
        seeds = [TOURNAMENT_MATCH_SEED, &tournament_id.to_le_bytes(), &match_index.to_le_bytes()],
        bump = tournament_match.bump
    )]
    pub tournament_match: Account<'info, TournamentMatch>,
    /// The tournament authority is the trusted source of match results (results
    /// are not verified against an on-chain game account by design).
    #[account(mut)]
    pub authority: Signer<'info>,
}

pub fn handler(
    ctx: Context<RecordMatchResult>,
    tournament_id: u64,
    match_index: u16,
    winner: Pubkey,
    loser: Pubkey,
) -> Result<()> {
    let tm = &mut ctx.accounts.tournament_match;
    require!(
        ctx.accounts.tournament.tournament_id == tournament_id,
        GameErrorCode::UnauthorizedAccess
    );
    require!(
        tm.match_index == match_index,
        GameErrorCode::InvalidMatchStatus
    );
    require!(
        tm.tournament_id == tournament_id,
        GameErrorCode::InvalidMatchStatus
    );
    guards::require_match_participants(tm, winner, loser)?;

    require!(
        tm.status == MatchStatus::Active || tm.status == MatchStatus::Pending,
        GameErrorCode::InvalidMatchStatus
    );

    tm.winner = Some(winner);
    tm.status = MatchStatus::Completed;
    tm.completed_at = Some(Clock::get()?.unix_timestamp);

    let tournament = &mut ctx.accounts.tournament;
    let final_idx = tournament.final_match_index;

    // Check if this is a semifinal (the two matches right before the final)
    let semifinal1_idx = final_idx.saturating_sub(2);
    let semifinal2_idx = final_idx.saturating_sub(1);

    if match_index == semifinal1_idx {
        tournament.fourth_place = Some(loser);
    } else if match_index == semifinal2_idx {
        tournament.third_place = Some(loser);
    } else if match_index == final_idx {
        // Final completed - tournament done
        tournament.winner = Some(winner);
        tournament.second_place = Some(loser);
        tournament.status = TournamentStatus::Completed;
        tournament.completed_at = Some(Clock::get()?.unix_timestamp);
    } else {
    }

    Ok(())
}

/// Instruction to advance a winner from a completed match to the next round.
/// Used by the backend to populate the next match after recording a result.
#[derive(Accounts)]
#[instruction(tournament_id: u64, source_match_index: u16, target_match_index: u16)]
pub struct AdvanceWinner<'info> {
    #[account(
        mut,
        seeds = [TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        bump = tournament.bump,
        constraint = tournament.authority == authority.key() @ GameErrorCode::NotTournamentAuthority
    )]
    pub tournament: Account<'info, Tournament>,
    #[account(
        seeds = [TOURNAMENT_MATCH_SEED, &tournament_id.to_le_bytes(), &source_match_index.to_le_bytes()],
        bump = source_match.bump
    )]
    pub source_match: Account<'info, TournamentMatch>,
    #[account(
        mut,
        seeds = [TOURNAMENT_MATCH_SEED, &tournament_id.to_le_bytes(), &target_match_index.to_le_bytes()],
        bump = target_match.bump
    )]
    pub target_match: Account<'info, TournamentMatch>,
    #[account(mut)]
    pub authority: Signer<'info>,
}

pub fn handler_advance_winner(
    ctx: Context<AdvanceWinner>,
    tournament_id: u64,
    source_match_index: u16,
) -> Result<()> {
    let source = &ctx.accounts.source_match;
    require!(
        ctx.accounts.tournament.tournament_id == tournament_id,
        GameErrorCode::UnauthorizedAccess
    );
    require!(
        source.match_index == source_match_index,
        GameErrorCode::InvalidMatchStatus
    );

    require!(
        source.status == MatchStatus::Completed,
        GameErrorCode::InvalidMatchStatus
    );

    let winner = source.winner.ok_or(GameErrorCode::InvalidMatchStatus)?;
    let target = &mut ctx.accounts.target_match;

    // Place winner in the correct slot
    if source.next_match_slot == 0 {
        target.player_white = Some(winner);
    } else {
        target.player_black = Some(winner);
    }

    // Update target status to Pending if both slots filled
    if target.player_white.is_some() && target.player_black.is_some() {
        target.status = MatchStatus::Pending;
    }

    Ok(())
}
