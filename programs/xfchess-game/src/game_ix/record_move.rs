use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(game_id: u64)]
pub struct RecordMove<'info> {
    #[account(mut, seeds = [GAME_SEED, &game_id.to_le_bytes()], bump)]
    pub game: Account<'info, Game>,
    #[account(mut, seeds = [MOVE_LOG_SEED, &game_id.to_le_bytes()], bump)]
    pub move_log: Account<'info, MoveLog>,
    pub player: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<RecordMove>,
    game_id: u64,
    move_str: String,
    next_fen: String,
    nonce: u64,
    signature: Option<Vec<u8>>,
) -> Result<()> {
    let game = &mut ctx.accounts.game;
    let move_log = &mut ctx.accounts.move_log;

    // Validate player is in the game
    require!(
        ctx.accounts.player.key() == game.white || ctx.accounts.player.key() == game.black,
        GameErrorCode::NotInGame
    );

    // Validate game state
    require!(
        game.status == GameStatus::Active,
        GameErrorCode::GameNotActive
    );

    // Validate it's player's turn
    let is_white_turn = move_log.moves.len() % 2 == 0;
    let player_color = if ctx.accounts.player.key() == game.white {
        true
    } else {
        false
    };
    require!(
        is_white_turn == player_color,
        GameErrorCode::NotYourTurn
    );

    // Record the move
    let move_index = move_log.moves.len();
    if move_index < move_log.moves.capacity() {
        move_log.moves[move_index] = move_str.clone();
        move_log.timestamps[move_index] = Clock::get()?.unix_timestamp;
        if let Some(sig) = signature {
            move_log.signatures[move_index] = sig;
        }
    } else {
        // Handle full move log - shift or expand if needed
        msg!("Move log full, appending with overflow logic");
        move_log.moves.push(move_str.clone());
        move_log.timestamps.push(Clock::get()?.unix_timestamp);
        if let Some(sig) = signature {
            move_log.signatures.push(sig);
        }
    }

    // Update game state
    game.last_move_timestamp = Clock::get()?.unix_timestamp;
    game.current_fen = next_fen;

    msg!("Move recorded for game {} by player {}", game_id, ctx.accounts.player.key());
    Ok(())
}
