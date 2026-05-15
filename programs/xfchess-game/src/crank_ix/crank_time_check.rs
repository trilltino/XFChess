//! Automatic time check crank instruction.
//! 
//! This instruction is called automatically by the MagicBlock scheduler
//! to check if a player has exceeded their time limit.
//! 
//! Must be sent to the Ephemeral Rollup.

use anchor_lang::prelude::*;
use crate::state::{Game, GameStatus, GameResult};

/// Empty instruction data for crank_time_check (no parameters needed)
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct CrankTimeCheckData {}

/// Automatic time check called by the scheduled crank.
/// 
/// This instruction:
/// 1. Checks if the game is active
/// 2. Calculates if the current player has exceeded their time limit
/// 3. Automatically sets the game result if timeout occurred
pub fn crank_time_check(ctx: Context<CrankTimeCheck>, _data: CrankTimeCheckData) -> Result<()> {
    let game = &mut ctx.accounts.game;
    let clock = Clock::get()?;
    let now = clock.unix_timestamp;
    
    // Only check active games
    if game.status != GameStatus::Active {
        return Ok(());
    }
    
    // Skip if no time control set
    if game.base_time_seconds == 0 {
        return Ok(());
    }
    
    // Calculate time elapsed since last move (3× base_time safety window)
    let time_elapsed = (now - game.updated_at) as u64;
    let time_limit = game.base_time_seconds.saturating_mul(3);
    
    // Check if time has expired
    if time_elapsed > time_limit {
        // Determine which player timed out based on whose turn it is
        // turn: 1, 3, 5... = white to move
        // turn: 2, 4, 6... = black to move
        let white_to_move = game.turn % 2 == 1;
        
        let timed_out_player = if white_to_move {
            game.white
        } else {
            game.black
        };
        
        let winner = if white_to_move {
            game.black
        } else {
            game.white
        };
        
        // Set game as finished with timeout result
        game.status = GameStatus::Finished;
        game.result = GameResult::Winner(winner);
        game.updated_at = now;
        
    } else {
    }
    
    Ok(())
}

#[derive(Accounts)]
pub struct CrankTimeCheck<'info> {
    /// The game account to check
    #[account(
        mut,
        seeds = [b"game", game.game_id.to_le_bytes().as_ref()],
        bump = game.bump,
    )]
    pub game: Account<'info, Game>,
    
    /// CHECK: White player (for reference)
    pub white: AccountInfo<'info>,
    
    /// CHECK: Black player (for reference)  
    pub black: AccountInfo<'info>,
}
