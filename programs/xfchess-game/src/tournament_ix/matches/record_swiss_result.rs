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
    /// TournamentPlayersShard 0 always present (all tournament sizes).
    /// `mut` is required — standings updates must persist.
    #[account(
        mut,
        seeds = [TOURNAMENT_PLAYERS_SEED, &[0u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_0: Account<'info, TournamentPlayersShard>,
    /// TournamentPlayersShard 1 — present for >64-player tournaments only.
    /// Pass the program ID in its place for smaller tournaments.
    #[account(
        mut,
        seeds = [TOURNAMENT_PLAYERS_SEED, &[1u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_1: Option<Account<'info, TournamentPlayersShard>>,
    /// TournamentPlayersShard 2 — present for 256-player tournaments only.
    #[account(
        mut,
        seeds = [TOURNAMENT_PLAYERS_SEED, &[2u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_2: Option<Account<'info, TournamentPlayersShard>>,
    /// TournamentPlayersShard 3 — present for 256-player tournaments only.
    #[account(
        mut,
        seeds = [TOURNAMENT_PLAYERS_SEED, &[3u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_3: Option<Account<'info, TournamentPlayersShard>>,

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
    require!(
        t.tournament_id == tournament_id,
        GameErrorCode::UnauthorizedAccess
    );
    let player = ctx.accounts.player.key();
    let opponent = ctx.accounts.opponent.key();

    require!(
        t.status == TournamentStatus::Active,
        GameErrorCode::InvalidGameStatus
    );
    require!(
        matches!(t.tournament_type, TournamentType::Swiss { .. }),
        GameErrorCode::InvalidGameStatus
    );
    require!(round == t.current_round, GameErrorCode::InvalidGameStatus);
    require!(round <= t.total_rounds, GameErrorCode::InvalidGameStatus);
    let boards_per_round = t.num_registered_players.max(2) / 2;
    require!(board < boards_per_round, GameErrorCode::InvalidArgument);

    // Collect the present shards (1-3 are optional — small/medium tournaments
    // only initialize shard 0 or 0-1). Presence is prefix-closed, so the
    // position in this vec equals the shard id.
    let mut shards: Vec<&mut TournamentPlayersShard> =
        vec![&mut ctx.accounts.tournament_players_shard_0];
    if let Some(s) = ctx.accounts.tournament_players_shard_1.as_mut() {
        shards.push(s);
    }
    if let Some(s) = ctx.accounts.tournament_players_shard_2.as_mut() {
        shards.push(s);
    }
    if let Some(s) = ctx.accounts.tournament_players_shard_3.as_mut() {
        shards.push(s);
    }

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
    let (opponent_shard_id, opponent_idx) =
        opponent_shard_idx.ok_or(GameErrorCode::PlayerNotFound)?;

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
        // Different shards — split the vec so we can hold both mutably.
        let lo = player_shard_id.min(opponent_shard_id);
        let hi = player_shard_id.max(opponent_shard_id);
        let (left, right) = shards.split_at_mut(hi);
        let (player_shard, opponent_shard) = if player_shard_id == lo {
            (&mut *left[lo], &mut *right[0])
        } else {
            (&mut *right[0], &mut *left[lo])
        };
        update_shards(
            player_shard,
            opponent_shard,
            player_idx,
            opponent_idx,
            result,
            player_score_before,
            opponent_score_before,
        );
    }

    // Round advancement is deliberately not inferred from board index. The
    // backend/authority must advance rounds only after every board result for
    // the round is known; otherwise a forged last-board call can skip results.

    msg!(
        "Swiss result recorded: player {} vs opponent {} result {:?}",
        player,
        opponent,
        result
    );

    Ok(())
}

fn update_shards(
    player_shard: &mut TournamentPlayersShard,
    opponent_shard: &mut TournamentPlayersShard,
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
