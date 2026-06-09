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
    /// TournamentPlayersShard 0 (players 0-63)
    #[account(
        seeds = [TOURNAMENT_PLAYERS_SEED, &[0u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_0: Account<'info, TournamentPlayersShard>,
    /// TournamentPlayersShard 1 (players 64-127)
    #[account(
        seeds = [TOURNAMENT_PLAYERS_SEED, &[1u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_1: Account<'info, TournamentPlayersShard>,
    /// TournamentPlayersShard 2 (players 128-191)
    #[account(
        seeds = [TOURNAMENT_PLAYERS_SEED, &[2u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_2: Account<'info, TournamentPlayersShard>,
    /// TournamentPlayersShard 3 (players 192-255)
    #[account(
        seeds = [TOURNAMENT_PLAYERS_SEED, &[3u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_3: Account<'info, TournamentPlayersShard>,

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
    require!(t.tournament_id == tournament_id, GameErrorCode::UnauthorizedAccess);
    let player = ctx.accounts.player.key();
    let opponent = ctx.accounts.opponent.key();

    require!(t.status == TournamentStatus::Active, GameErrorCode::InvalidGameStatus);
    require!(
        matches!(t.tournament_type, TournamentType::Swiss { .. }),
        GameErrorCode::InvalidGameStatus
    );
    require!(round == t.current_round, GameErrorCode::InvalidGameStatus);
    require!(round <= t.total_rounds, GameErrorCode::InvalidGameStatus);

    // Collect all shards to search for players
    let mut shards = [
        &mut ctx.accounts.tournament_players_shard_0,
        &mut ctx.accounts.tournament_players_shard_1,
        &mut ctx.accounts.tournament_players_shard_2,
        &mut ctx.accounts.tournament_players_shard_3,
    ];

    // Find player and opponent indices across all shards
    let mut player_shard_idx: Option<(usize, usize)> = None; // (shard_id, player_idx)
    let mut opponent_shard_idx: Option<(usize, usize)> = None;

    for (shard_idx, shard) in shards.iter().enumerate() {
        for (player_idx, standing) in shard.swiss_standings.iter().enumerate() {
            if standing.player == player {
                player_shard_idx = Some((shard_idx, player_idx));
            }
            if standing.player == opponent {
                opponent_shard_idx = Some((shard_idx, player_idx));
            }
        }
    }

    let (player_shard_id, player_idx) = player_shard_idx.ok_or(GameErrorCode::PlayerNotFound)?;
    let (opponent_shard_id, opponent_idx) = opponent_shard_idx.ok_or(GameErrorCode::PlayerNotFound)?;

    // Get scores before updating
    let player_score_before = shards[player_shard_id].swiss_standings[player_idx].score;
    let opponent_score_before = shards[opponent_shard_id].swiss_standings[opponent_idx].score;

    // Handle updates based on whether players are in same or different shards
    if player_shard_id == opponent_shard_id {
        // Same shard - use single mutable reference
        let shard = &mut shards[player_shard_id];
        
        // Update scores based on result
        match result {
            SwissMatchResult::Win => {
                shard.swiss_standings[player_idx].score += 2;
                shard.swiss_standings[opponent_idx].score += 0;
                shard.swiss_standings[player_idx].color_balance += 1;
                shard.swiss_standings[opponent_idx].color_balance -= 1;
            }
            SwissMatchResult::Loss => {
                shard.swiss_standings[player_idx].score += 0;
                shard.swiss_standings[opponent_idx].score += 2;
                shard.swiss_standings[player_idx].color_balance -= 1;
                shard.swiss_standings[opponent_idx].color_balance += 1;
            }
            SwissMatchResult::Draw => {
                shard.swiss_standings[player_idx].score += 1;
                shard.swiss_standings[opponent_idx].score += 1;
                // No color change for draws
            }
        }

        // Update tiebreakers
        shard.swiss_standings[player_idx].buchholz += opponent_score_before as u16;
        shard.swiss_standings[opponent_idx].buchholz += player_score_before as u16;

        if result == SwissMatchResult::Win {
            shard.swiss_standings[player_idx].sonneborn += opponent_score_before as u16;
        } else if result == SwissMatchResult::Loss {
            shard.swiss_standings[opponent_idx].sonneborn += player_score_before as u16;
        } else {
            // Draw - both get half of opponent's score
            shard.swiss_standings[player_idx].sonneborn += (opponent_score_before / 2) as u16;
            shard.swiss_standings[opponent_idx].sonneborn += (player_score_before / 2) as u16;
        }
    } else {
        // Different shards - use match to handle each pair
        match (player_shard_id, opponent_shard_id) {
            (0, 1) | (1, 0) => {
                let (s0, s1) = shards.split_at_mut(1);
                let (player_shard, opponent_shard) = if player_shard_id == 0 {
                    (&mut s0[0], &mut s1[0])
                } else {
                    (&mut s1[0], &mut s0[0])
                };
                update_shards(player_shard, opponent_shard, player_idx, opponent_idx, result, player_score_before, opponent_score_before);
            }
            (0, 2) | (2, 0) => {
                let (s0, s2) = shards.split_at_mut(2);
                let (player_shard, opponent_shard) = if player_shard_id == 0 {
                    (&mut s0[0], &mut s2[0])
                } else {
                    (&mut s2[0], &mut s0[0])
                };
                update_shards(player_shard, opponent_shard, player_idx, opponent_idx, result, player_score_before, opponent_score_before);
            }
            (0, 3) | (3, 0) => {
                let (s0, s3) = shards.split_at_mut(3);
                let (player_shard, opponent_shard) = if player_shard_id == 0 {
                    (&mut s0[0], &mut s3[0])
                } else {
                    (&mut s3[0], &mut s0[0])
                };
                update_shards(player_shard, opponent_shard, player_idx, opponent_idx, result, player_score_before, opponent_score_before);
            }
            (1, 2) | (2, 1) => {
                let (s1, s2) = shards.split_at_mut(2);
                let (player_shard, opponent_shard) = if player_shard_id == 1 {
                    (&mut s1[0], &mut s2[0])
                } else {
                    (&mut s2[0], &mut s1[0])
                };
                update_shards(player_shard, opponent_shard, player_idx, opponent_idx, result, player_score_before, opponent_score_before);
            }
            (1, 3) | (3, 1) => {
                let (s1, s3) = shards.split_at_mut(3);
                let (player_shard, opponent_shard) = if player_shard_id == 1 {
                    (&mut s1[0], &mut s3[0])
                } else {
                    (&mut s3[0], &mut s1[0])
                };
                update_shards(player_shard, opponent_shard, player_idx, opponent_idx, result, player_score_before, opponent_score_before);
            }
            (2, 3) | (3, 2) => {
                let (s2, s3) = shards.split_at_mut(3);
                let (player_shard, opponent_shard) = if player_shard_id == 2 {
                    (&mut s2[0], &mut s3[0])
                } else {
                    (&mut s3[0], &mut s2[0])
                };
                update_shards(player_shard, opponent_shard, player_idx, opponent_idx, result, player_score_before, opponent_score_before);
            }
            _ => return Err(GameErrorCode::PlayerNotFound.into()),
        }
    }

    // Auto-advance round when the last board of the current round is recorded
    let boards_per_round = t.max_players / 2;
    if board == boards_per_round - 1 {
        if t.current_round < t.total_rounds {
            t.current_round += 1;
        } else {
            t.status = TournamentStatus::Completed;
            t.completed_at = Some(Clock::get()?.unix_timestamp);
        }
    }

    msg!("Swiss result recorded: player {} vs opponent {} result {:?}", player, opponent, result);

    Ok(())
}

fn update_shards(
    player_shard: &mut &mut Account<TournamentPlayersShard>,
    opponent_shard: &mut &mut Account<TournamentPlayersShard>,
    player_idx: usize,
    opponent_idx: usize,
    result: SwissMatchResult,
    player_score_before: u8,
    opponent_score_before: u8,
) {
    // Update scores based on result
    match result {
        SwissMatchResult::Win => {
            player_shard.swiss_standings[player_idx].score += 2;
            opponent_shard.swiss_standings[opponent_idx].score += 0;
            player_shard.swiss_standings[player_idx].color_balance += 1;
            opponent_shard.swiss_standings[opponent_idx].color_balance -= 1;
        }
        SwissMatchResult::Loss => {
            player_shard.swiss_standings[player_idx].score += 0;
            opponent_shard.swiss_standings[opponent_idx].score += 2;
            player_shard.swiss_standings[player_idx].color_balance -= 1;
            opponent_shard.swiss_standings[opponent_idx].color_balance += 1;
        }
        SwissMatchResult::Draw => {
            player_shard.swiss_standings[player_idx].score += 1;
            opponent_shard.swiss_standings[opponent_idx].score += 1;
            // No color change for draws
        }
    }

    // Update tiebreakers
    player_shard.swiss_standings[player_idx].buchholz += opponent_score_before as u16;
    opponent_shard.swiss_standings[opponent_idx].buchholz += player_score_before as u16;

    if result == SwissMatchResult::Win {
        player_shard.swiss_standings[player_idx].sonneborn += opponent_score_before as u16;
    } else if result == SwissMatchResult::Loss {
        opponent_shard.swiss_standings[opponent_idx].sonneborn += player_score_before as u16;
    } else {
        // Draw - both get half of opponent's score
        player_shard.swiss_standings[player_idx].sonneborn += (opponent_score_before / 2) as u16;
        opponent_shard.swiss_standings[opponent_idx].sonneborn += (player_score_before / 2) as u16;
    }
}
