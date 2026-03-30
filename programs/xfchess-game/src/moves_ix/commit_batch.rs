use crate::errors::XfchessGameError;
use crate::state::{Game, GameStatus, SessionDelegation};
use crate::state::move_log::MoveLog;
use anchor_lang::prelude::*;

#[cfg(feature = "move-validation")]
use chess_logic_on_chain::shakmaty::{fen::Fen, uci::UciMove, CastlingMode, Chess, EnPassantMode, Position};

#[derive(Accounts)]
pub struct CommitMoveBatchCtx<'info> {
    #[account(
        mut,
        seeds = [b"game", game.game_id.to_le_bytes().as_ref()],
        bump,
    )]
    pub game: Account<'info, Game>,

    #[account(
        mut,
        seeds = [b"move_log", game.game_id.to_le_bytes().as_ref()],
        bump,
    )]
    pub move_log: Account<'info, MoveLog>,

    /// CHECK: Verified against session_key in white_delegation
    #[account(mut)]
    pub white_session: Signer<'info>,

    /// CHECK: Verified against session_key in black_delegation
    #[account(mut)]
    pub black_session: Signer<'info>,

    #[account(
        mut,
        seeds = [b"session_delegation", game.game_id.to_le_bytes().as_ref(), game.white.as_ref()],
        bump = white_delegation.bump,
    )]
    pub white_delegation: Account<'info, SessionDelegation>,

    #[account(
        mut,
        seeds = [b"session_delegation", game.game_id.to_le_bytes().as_ref(), game.black.as_ref()],
        bump = black_delegation.bump,
    )]
    pub black_delegation: Account<'info, SessionDelegation>,

    pub system_program: Program<'info, System>,
}

pub fn handler_commit_move_batch(
    ctx: Context<CommitMoveBatchCtx>,
    _game_id: u64,
    moves: Vec<String>,
    next_fens: Vec<String>,
) -> Result<()> {
    let game = &mut ctx.accounts.game;
    let move_log = &mut ctx.accounts.move_log;
    let white_delegation = &ctx.accounts.white_delegation;
    let black_delegation = &ctx.accounts.black_delegation;
    let clock = Clock::get()?;

    require!(
        game.status == GameStatus::Active,
        XfchessGameError::GameNotActive
    );
    require!(
        moves.len() == next_fens.len(),
        XfchessGameError::InvalidBatchLength
    );
    require!(
        moves.len() <= white_delegation.max_batch_len as usize,
        XfchessGameError::BatchTooLarge
    );
    require!(
        ctx.accounts.white_session.key() == white_delegation.session_key,
        XfchessGameError::InvalidSessionKey
    );
    require!(
        ctx.accounts.black_session.key() == black_delegation.session_key,
        XfchessGameError::InvalidSessionKey
    );
    require!(
        white_delegation.enabled && clock.unix_timestamp <= white_delegation.expires_at,
        XfchessGameError::SessionExpiredOrDisabled
    );
    require!(
        black_delegation.enabled && clock.unix_timestamp <= black_delegation.expires_at,
        XfchessGameError::SessionExpiredOrDisabled
    );

    #[cfg(feature = "move-validation")]
    {
        let initial_fen =
            Fen::from_ascii(game.fen.as_bytes()).map_err(|_| XfchessGameError::InvalidBoardState)?;
        let mut current_pos: Chess = initial_fen
            .into_position(CastlingMode::Standard)
            .map_err(|_| XfchessGameError::InvalidBoardState)?;

        for (move_str, next_fen_str) in moves.iter().zip(next_fens.iter()) {
            let uci: UciMove = move_str
                .parse()
                .map_err(|_| XfchessGameError::InvalidMove)?;
            let chess_move = uci
                .to_move(&current_pos)
                .map_err(|_| XfchessGameError::InvalidMove)?;
            let new_pos = current_pos
                .play(&chess_move)
                .map_err(|_| XfchessGameError::InvalidMove)?;

            let computed_fen = Fen::from_position(new_pos.clone(), EnPassantMode::Legal);

            let provided_fen = Fen::from_ascii(next_fen_str.as_bytes())
                .map_err(|_| XfchessGameError::InvalidNextFen)?;

            require!(
                computed_fen.to_string() == provided_fen.to_string(),
                XfchessGameError::InvalidNextFen
            );

            current_pos = new_pos;
            move_log
                .moves
                .push(format!("{}. {}", game.move_count / 2 + 1, move_str));
        }

        game.fen = Fen::from_position(current_pos, EnPassantMode::Legal).to_string();
    }

    #[cfg(not(feature = "move-validation"))]
    {
        // Without shakmaty, just store the last provided FEN and moves
        if let Some(last_fen) = next_fens.last() {
            game.fen = last_fen.clone();
        }
        for move_str in moves.iter() {
            move_log
                .moves
                .push(format!("{}. {}", game.move_count / 2 + 1, move_str));
        }
    }
    game.move_count += moves.len() as u16;
    game.updated_at = clock.unix_timestamp;

    Ok(())
}
