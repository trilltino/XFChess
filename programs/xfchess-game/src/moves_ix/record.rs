//! Instruction for validating and recording a single state-transitioning chess move.

use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;

#[cfg(feature = "move-validation")]
use chess_logic_on_chain::nimzovich_engine::{CompactBoard, MoveOutcome, parse_uci, validate_and_apply};

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

    require!(
        game.status == GameStatus::Active,
        GameErrorCode::GameNotActive
    );
    
    // Check session expiration
    require!(
        Clock::get()?.unix_timestamp <= ctx.accounts.session_delegation.expires_at,
        GameErrorCode::SessionExpired
    );

    let expected_player = if game.turn % 2 == 1 { game.white } else { game.black };
    require!(
        expected_player == _moving_player,
        GameErrorCode::NotYourTurn
    );

    // Causal chain check: parent_nonce must equal the current game.nonce before advancing.
    if let Some(pn) = parent_nonce {
        require!(pn == game.nonce, GameErrorCode::ParentNonceMismatch);
    }

    // Replay Protection (Nonce stored directly in Game account)
    require!(nonce == game.nonce + 1, GameErrorCode::InvalidNonce);
    game.nonce = nonce;

    // Increment fees_advanced for platform reimbursement
    game.fees_advanced = game.fees_advanced.checked_add(RECORD_RESULT_COST).ok_or(GameErrorCode::ArithmeticOverflow)?;

    // --- ON-CHAIN CHESS VALIDATION ---
    #[cfg(feature = "move-validation")]
    {
        // 1. Parse current board state from stored binary blob (zero-copy cast)
        let cb = CompactBoard::from_bytes(&game.board_state);
        let mut on_chain_game = cb.to_on_chain_game();

        // 2. Parse incoming UCI move string & validate legality
        let outcome = validate_and_apply(&mut on_chain_game, &move_uci)
            .map_err(|_| GameErrorCode::InvalidMove)?;

        // 3. Verify the client's provided next_board perfectly matches the applied move consequence
        let expected_next_cb = on_chain_game.to_compact_board();
        let client_next_cb = CompactBoard::from_bytes(&next_board);

        require!(
            expected_next_cb == client_next_cb,
            GameErrorCode::InvalidBoardState
        );

        // 4. Halfmove clock for the 50-move rule: reset on a pawn move or a
        //    capture (the only irreversible events), otherwise increment. `cb`
        //    is the board *before* the move, so we read the moved piece and the
        //    captured square from it.
        let (src, dst, _promo) = parse_uci(&move_uci).map_err(|_| GameErrorCode::InvalidMove)?;
        let moved_piece = cb.squares[src as usize];
        let is_pawn_move = moved_piece.abs() == 1;
        // A diagonal pawn move is always a capture (covers en passant, where the
        // destination square itself is empty before the move).
        let is_capture = cb.squares[dst as usize] != 0
            || (is_pawn_move && (src % 8) != (dst % 8));
        if is_pawn_move || is_capture {
            game.halfmove_clock = 0;
        } else {
            game.halfmove_clock = game.halfmove_clock.saturating_add(1);
        }

        // 5. Auto-detect game end: mate, stalemate, dead position, or 50-move rule.
        match outcome {
            MoveOutcome::Checkmate => {
                game.result = GameResult::Winner(_moving_player);
                game.status = GameStatus::Finished;
            }
            MoveOutcome::Stalemate | MoveOutcome::InsufficientMaterial => {
                game.result = GameResult::Draw;
                game.status = GameStatus::Finished;
            }
            MoveOutcome::Playing => {
                // 50-move rule: 100 half-moves with no pawn move or capture.
                if game.halfmove_clock >= 100 {
                    game.result = GameResult::Draw;
                    game.status = GameStatus::Finished;
                }
            }
        }
    }

    game.board_state = next_board;
    game.move_count += 1;
    game.turn += 1;
    let timestamp = Clock::get()?.unix_timestamp;
    game.updated_at = timestamp;

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
