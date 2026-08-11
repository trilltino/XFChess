//! Instruction for recording a move via the *global* (per-wallet) session key,
//! mirroring `record.rs` for games created with `global_create_game`/
//! `global_join_game`. Those never get a per-game `SessionDelegation` account
//! (that's what makes their create/join popup-free), so the original
//! `record_move` — which hard-requires that account — fails on-chain for
//! them. Same move-application logic (`apply::apply_recorded_move`), just
//! checked against `GlobalSessionDelegation` instead.

use crate::constants::GAME_SEED;
use crate::errors::GameErrorCode;
use crate::moves_ix::apply;
use crate::state::*;
use anchor_lang::prelude::*;

/// Accounts for recording a move via a wallet's global session key.
/// `player` is the session signer, not the wallet — the actual moving
/// wallet is `session_delegation.player`, which must be one of `game.white`/
/// `game.black` (a global session isn't scoped to one game the way a
/// per-game `SessionDelegation` is, so that check has to happen here
/// instead of being implicit in the PDA's seeds).
#[derive(Accounts)]
#[instruction(game_id: u64)]
pub struct GlobalRecordMove<'info> {
    #[account(mut, seeds = [GAME_SEED, &game_id.to_le_bytes()], bump)]
    pub game: Account<'info, Game>,
    /// Global session key — must match the session_delegation below.
    pub player: Signer<'info>,
    /// Global session delegation for the wallet actually making this move.
    #[account(
        seeds = [GlobalSessionDelegation::SEED, session_delegation.player.as_ref()],
        bump = session_delegation.bump,
        constraint = session_delegation.session_key == player.key() @ GameErrorCode::InvalidSessionKey,
        constraint = session_delegation.enabled @ GameErrorCode::SessionExpiredOrDisabled,
        constraint = session_delegation.player == game.white || session_delegation.player == game.black
            @ GameErrorCode::NotInGame,
    )]
    pub session_delegation: Account<'info, GlobalSessionDelegation>,
}

/// Same shape as `record::handler` — checks session expiry, delegates to the
/// shared `apply::apply_recorded_move`, emits the same `MoveEvent`.
pub fn handler(
    ctx: Context<GlobalRecordMove>,
    _game_id: u64,
    move_uci: [u8; 5],
    next_board: [u8; 68],
    nonce: u64,
    _signature: Option<Vec<u8>>,
    parent_nonce: Option<u64>,
) -> Result<()> {
    let moving_player = ctx.accounts.session_delegation.player;
    let game = &mut ctx.accounts.game;

    require!(
        Clock::get()?.unix_timestamp <= ctx.accounts.session_delegation.expires_at,
        GameErrorCode::SessionExpired
    );

    let timestamp = Clock::get()?.unix_timestamp;
    apply::apply_recorded_move(
        game,
        moving_player,
        move_uci,
        next_board,
        nonce,
        parent_nonce,
        timestamp,
    )?;

    // See `record::handler`'s matching comment — plain-text FEN for explorers
    // that can't decode the `MoveEvent` below without this program's IDL.
    msg!(
        "FEN: {}",
        chess_logic_on_chain::nimzovich_engine::CompactBoard::from_bytes(&next_board).to_fen()
    );

    emit!(crate::events::MoveEvent {
        game_id: _game_id,
        player: moving_player,
        move_uci,
        move_number: game.move_count,
        board_state: next_board,
        timestamp,
    });

    Ok(())
}
