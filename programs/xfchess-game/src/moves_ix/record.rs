//! Instruction for validating and recording a single state-transitioning chess move.

use crate::constants::GAME_SEED;
use crate::errors::GameErrorCode;
use crate::moves_ix::apply;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(game_id: u64)]
pub struct RecordMove<'info> {
    #[account(mut, seeds = [GAME_SEED, &game_id.to_le_bytes()], bump)]
    pub game: Account<'info, Game>,
    /// Session key — must match the session_delegation registered for this game.
    pub player: Signer<'info>,
    /// Session delegation account linking session_key → wallet for this game.
    #[account(
        seeds = [
            b"session_delegation",
            &game_id.to_le_bytes(),
            session_delegation.player.as_ref(),
        ],
        bump = session_delegation.bump,
        constraint = session_delegation.session_key == player.key() @ GameErrorCode::InvalidSessionKey,
        constraint = session_delegation.enabled @ GameErrorCode::SessionExpiredOrDisabled,
    )]
    pub session_delegation: Account<'info, SessionDelegation>,
}

pub fn handler(
    ctx: Context<RecordMove>,
    _game_id: u64,
    move_uci: [u8; 5],
    next_board: [u8; 68],
    nonce: u64,
    _signature: Option<Vec<u8>>,
    // Causal chain: client's claimed game.nonce before this move.
    // Must equal game.nonce if provided; None = legacy client (skip check).
    parent_nonce: Option<u64>,
) -> Result<()> {
    // Capture before mutable borrows to avoid split-borrow issues inside cfg blocks
    let _moving_player = ctx.accounts.session_delegation.player;
    let game = &mut ctx.accounts.game;

    // Check session expiration
    require!(
        Clock::get()?.unix_timestamp <= ctx.accounts.session_delegation.expires_at,
        GameErrorCode::SessionExpired
    );

    let timestamp = Clock::get()?.unix_timestamp;
    apply::apply_recorded_move(
        game,
        _moving_player,
        move_uci,
        next_board,
        nonce,
        parent_nonce,
        timestamp,
    )?;

    // Emit MoveEvent for Ledger-based history tracking (zero rent cost)
    emit!(crate::events::MoveEvent {
        game_id: _game_id,
        player: _moving_player,
        move_uci,
        move_number: game.move_count,
        board_state: next_board,
        timestamp,
    });

    Ok(())
}
