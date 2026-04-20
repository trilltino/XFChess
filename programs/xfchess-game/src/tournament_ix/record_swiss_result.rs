//! Instruction to record a Swiss tournament match result and update standings.

use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(tournament_id: u64, round: u8, board: u16)]
pub struct RecordSwissResult<'info> {
    #[account(
        mut,
        seeds = [TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        bump = tournament.bump
    )]
    pub tournament: Account<'info, Tournament>,

    /// CHECK: Player who played the match
    #[account(mut)]
    pub player: Signer<'info>,

    /// CHECK: Opponent in the match
    pub opponent: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum SwissMatchResult {
    Win,
    Loss,
    Draw,
}

pub fn handler(
    ctx: Context<RecordSwissResult>,
    tournament_id: u64,
    round: u8,
    board: u16,
    result: SwissMatchResult,
) -> Result<()> {
    let t = &mut ctx.accounts.tournament;
    let player = ctx.accounts.player.key();
    let opponent = ctx.accounts.opponent.key();

    require!(t.status == TournamentStatus::Active, GameErrorCode::InvalidGameStatus);
    require!(
        matches!(t.tournament_type, TournamentType::Swiss { .. }),
        GameErrorCode::InvalidGameStatus
    );
    require!(round == t.current_round, GameErrorCode::InvalidGameStatus);
    require!(round <= t.total_rounds, GameErrorCode::InvalidGameStatus);

    // Find indices of players
    let player_idx = t.swiss_standings.iter()
        .position(|s| s.player == player)
        .ok_or_else(|| GameErrorCode::PlayerNotFound)?;

    let opponent_idx = t.swiss_standings.iter()
        .position(|s| s.player == opponent)
        .ok_or_else(|| GameErrorCode::PlayerNotFound)?;

    // Get scores before updating
    let player_score_before = t.swiss_standings[player_idx].score;
    let opponent_score_before = t.swiss_standings[opponent_idx].score;

    // Update scores based on result
    match result {
        SwissMatchResult::Win => {
            t.swiss_standings[player_idx].score += 2;
            t.swiss_standings[opponent_idx].score += 0;
            t.swiss_standings[player_idx].color_balance += 1;
            t.swiss_standings[opponent_idx].color_balance -= 1;
        }
        SwissMatchResult::Loss => {
            t.swiss_standings[player_idx].score += 0;
            t.swiss_standings[opponent_idx].score += 2;
            t.swiss_standings[player_idx].color_balance -= 1;
            t.swiss_standings[opponent_idx].color_balance += 1;
        }
        SwissMatchResult::Draw => {
            t.swiss_standings[player_idx].score += 1;
            t.swiss_standings[opponent_idx].score += 1;
            // No color change for draws
        }
    }

    // Update Buchholz (sum of opponents' scores)
    t.swiss_standings[player_idx].buchholz += opponent_score_before as u16;
    t.swiss_standings[opponent_idx].buchholz += player_score_before as u16;

    // Update Sonneborn-Berger (sum of defeated opponents' scores + 0.5*draws)
    match result {
        SwissMatchResult::Win => {
            t.swiss_standings[player_idx].sonneborn += opponent_score_before as u16;
        }
        SwissMatchResult::Draw => {
            t.swiss_standings[player_idx].sonneborn += (opponent_score_before / 2) as u16;
            t.swiss_standings[opponent_idx].sonneborn += (player_score_before / 2) as u16;
        }
        SwissMatchResult::Loss => {
            t.swiss_standings[opponent_idx].sonneborn += player_score_before as u16;
        }
    }

    msg!(
        "Swiss result recorded: Tournament {} Round {} Board {}: {:?} - Player {} vs {}",
        tournament_id, round, board, result, player, opponent
    );

    Ok(())
}
