# Crank Instructions

Solana program instructions for crank operations, including advancing tournament rounds, timing out games, and auto-starting tournaments.

## Overview

The crank instructions module provides automated maintenance operations for tournaments. Cranks are typically run by automated services (cron jobs, keepers) to ensure tournaments progress smoothly even when participants are inactive.

## Why Cranks?

Automated cranks are essential for:
- **Tournament progression** - Automatically advance rounds when matches complete
- **Game timeouts** - End games that have stalled due to inactivity
- **Tournament auto-start** - Begin tournaments when enough players register
- **Fairness** - Ensure tournaments don't stall due to participant inactivity

## Crank Architecture

Crank operations are designed to be:
- **Permissionless** - Anyone can call crank instructions
- **Idempotent** - Safe to call multiple times
- **Validated** - Only execute when conditions are met
- **Gas-efficient** - Minimal computation on-chain

## Components

- Advancing tournament rounds when matches complete
- Timing out games that exceed time limits
- Auto-starting tournaments when player count is reached
- Prize distribution for completed tournaments

## Example: Advancing Tournament Round

This instruction advances a tournament to the next round when all matches in the current round are complete.

```rust
use anchor_lang::prelude::*;

/// Context for advancing tournament round
#[derive(Accounts)]
pub struct AdvanceRound<'info> {
    #[account(mut)]
    pub tournament: Account<'info, Tournament>,
    
    /// CHECK: Crank authority (optional, for permissionless cranks)
    pub crank_authority: UncheckedAccount<'info>,
}

/// Advances tournament to the next round
/// 
/// This instruction:
/// 1. Validates tournament is in progress
/// 2. Checks if current round is complete
/// 3. Advances to next round
/// 4. Marks tournament complete if final round ends
/// 
/// # Arguments
/// * `ctx` - The instruction context
/// 
/// # Errors
/// - `TournamentNotInProgress` - Tournament is not currently in progress
/// - `RoundNotComplete` - Not all matches in current round are complete
pub fn advance_round(ctx: Context<AdvanceRound>) -> Result<()> {
    let tournament = &mut ctx.accounts.tournament;
    
    // Validate tournament is in progress
    require!(
        tournament.status == TournamentStatus::InProgress,
        ErrorCode::TournamentNotInProgress
    );
    
    // Check if current round is complete
    require!(
        round_complete(tournament),
        ErrorCode::RoundNotComplete
    );
    
    // Advance to next round
    tournament.round += 1;
    
    // Check if tournament is complete
    if tournament.round > 2 {
        tournament.status = TournamentStatus::Completed;
        tournament.completed_at = Clock::get()?.unix_timestamp;
    }
    
    msg!("Advanced to round {}", tournament.round);
    Ok(())
}

/// Checks if all matches in the current round are complete
/// 
/// # Arguments
/// * `tournament` - The tournament to check
/// 
/// # Returns
/// true if all matches in the current round are complete
fn round_complete(tournament: &Tournament) -> bool {
    // Check if all matches in current round are complete
    // Implementation depends on match tracking
    true // Placeholder
}
```

## Example: Timing Out Games

This instruction ends games that have exceeded their time limit.

```rust
/// Context for timing out a game
#[derive(Accounts)]
pub struct TimeoutGame<'info> {
    #[account(mut)]
    pub game: Account<'info, Game>,
    
    #[account(mut)]
    pub tournament: Account<'info, Tournament>,
    
    /// CHECK: Crank authority (optional)
    pub crank_authority: UncheckedAccount<'info>,
    
    pub clock: Sysvar<'info, Clock>,
}

/// Times out a game due to inactivity
/// 
/// This instruction:
/// 1. Checks if the game has exceeded the time limit
/// 2. Declares the inactive player as the loser
/// 3. Records the match result
/// 4. Updates tournament state
/// 
/// # Arguments
/// * `ctx` - The instruction context
/// * `timeout_seconds` - Time limit for the game in seconds
/// 
/// # Errors
/// - `GameNotActive` - Game is not currently active
/// - `TimeLimitNotReached` - Game has not exceeded time limit
pub fn timeout_game(
    ctx: Context<TimeoutGame>,
    timeout_seconds: i64,
) -> Result<()> {
    let game = &mut ctx.accounts.game;
    let current_time = Clock::get()?.unix_timestamp;
    
    // Validate game is active
    require!(
        game.status == GameStatus::Active,
        ErrorCode::GameNotActive
    );
    
    // Check if game has timed out
    let time_since_last_move = current_time - game.last_move_time;
    require!(
        time_since_last_move > timeout_seconds,
        ErrorCode::TimeLimitNotReached
    );
    
    // Determine winner based on last move
    let loser = game.current_turn;
    let winner = match loser {
        PlayerColor::White => PlayerColor::Black,
        PlayerColor::Black => PlayerColor::White,
    };
    
    // Update game state
    game.status = GameStatus::Completed;
    game.winner = Some(winner);
    game.end_reason = GameEndReason::Timeout;
    game.ended_at = current_time;
    
    // Update tournament if applicable
    if let Some(tournament_id) = game.tournament_id {
        // Update tournament match result
        // Implementation depends on tournament structure
    }
    
    msg!("Game timed out. Winner: {:?}", winner);
    Ok(())
}
```

## Example: Auto-Starting Tournament

This instruction automatically starts a tournament when enough players have registered.

```rust
/// Context for auto-starting a tournament
#[derive(Accounts)]
pub struct AutoStartTournament<'info> {
    #[account(mut)]
    pub tournament: Account<'info, Tournament>,
    
    /// CHECK: Crank authority (optional)
    pub crank_authority: UncheckedAccount<'info>,
    
    pub clock: Sysvar<'info, Clock>,
}

/// Automatically starts a tournament when conditions are met
/// 
/// This instruction:
/// 1. Validates tournament is open
/// 2. Checks if minimum players reached
/// 3. Generates the match bracket
/// 4. Sets tournament status to InProgress
/// 
/// # Arguments
/// * `ctx` - The instruction context
/// 
/// # Errors
/// - `TournamentNotOpen` - Tournament is not accepting registrations
/// - `NotEnoughPlayers` - Minimum player count not reached
pub fn auto_start_tournament(ctx: Context<AutoStartTournament>) -> Result<()> {
    let tournament = &mut ctx.accounts.tournament;
    let current_time = Clock::get()?.unix_timestamp;
    
    // Validate tournament is open
    require!(
        tournament.status == TournamentStatus::Open,
        ErrorCode::TournamentNotOpen
    );
    
    // Check if minimum players reached (2 for a match)
    require!(
        tournament.player_count >= 2,
        ErrorCode::NotEnoughPlayers
    );
    
    // Auto-start tournament
    tournament.status = TournamentStatus::InProgress;
    tournament.round = 1;
    tournament.started_at = current_time;
    
    // Generate bracket
    generate_bracket(tournament)?;
    
    msg!("Tournament auto-started with {} players", tournament.player_count);
    Ok(())
}

/// Generates tournament bracket
/// 
/// # Arguments
/// * `tournament` - The tournament to generate bracket for
/// 
/// # Returns
/// Ok(()) if bracket generation succeeds
fn generate_bracket(tournament: &mut Tournament) -> Result<()> {
    // Generate tournament bracket based on registered players
    // Implementation depends on player tracking
    Ok(())
}
```
