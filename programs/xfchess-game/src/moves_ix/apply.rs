//! Pure move application and bookkeeping for `record_move`.

use crate::constants::RECORD_RESULT_COST;
use crate::errors::GameErrorCode;
use crate::state::{Game, GameResult, GameStatus};
use anchor_lang::prelude::*;

#[cfg(feature = "move-validation")]
use chess_logic_on_chain::nimzovich_engine::{
    parse_uci, validate_and_apply, CompactBoard, MoveOutcome,
};

pub fn apply_recorded_move(
    game: &mut Game,
    moving_player: Pubkey,
    move_uci: [u8; 5],
    next_board: [u8; 68],
    nonce: u64,
    parent_nonce: Option<u64>,
    now: i64,
) -> Result<()> {
    require!(
        game.status == GameStatus::Active,
        GameErrorCode::GameNotActive
    );

    let expected_player = if game.turn % 2 == 1 {
        game.white
    } else {
        game.black
    };
    require!(expected_player == moving_player, GameErrorCode::NotYourTurn);

    if let Some(parent_nonce) = parent_nonce {
        require!(
            parent_nonce == game.nonce,
            GameErrorCode::ParentNonceMismatch
        );
    }

    let expected_nonce = game
        .nonce
        .checked_add(1)
        .ok_or(GameErrorCode::ArithmeticOverflow)?;
    require!(nonce == expected_nonce, GameErrorCode::InvalidNonce);
    game.nonce = nonce;

    game.fees_advanced = game
        .fees_advanced
        .checked_add(RECORD_RESULT_COST)
        .ok_or(GameErrorCode::ArithmeticOverflow)?;

    #[cfg(feature = "move-validation")]
    {
        let cb = CompactBoard::from_bytes(&game.board_state);
        let mut on_chain_game = cb.to_on_chain_game();

        let outcome = validate_and_apply(&mut on_chain_game, &move_uci)
            .map_err(|_| GameErrorCode::InvalidMove)?;

        let expected_next_cb = on_chain_game.to_compact_board();
        let client_next_cb = CompactBoard::from_bytes(&next_board);
        require!(
            expected_next_cb == client_next_cb,
            GameErrorCode::InvalidBoardState
        );

        let (src, dst, _promo) = parse_uci(&move_uci).map_err(|_| GameErrorCode::InvalidMove)?;
        let moved_piece = cb.squares[src as usize];
        let is_pawn_move = moved_piece.abs() == 1;
        let is_capture = cb.squares[dst as usize] != 0 || (is_pawn_move && (src % 8) != (dst % 8));
        if is_pawn_move || is_capture {
            game.halfmove_clock = 0;
        } else {
            game.halfmove_clock = game
                .halfmove_clock
                .checked_add(1)
                .ok_or(GameErrorCode::ArithmeticOverflow)?;
        }

        match outcome {
            MoveOutcome::Checkmate => {
                game.result = GameResult::Winner(moving_player);
                game.status = GameStatus::Finished;
            }
            MoveOutcome::Stalemate | MoveOutcome::InsufficientMaterial => {
                game.result = GameResult::Draw;
                game.status = GameStatus::Finished;
            }
            MoveOutcome::Playing => {
                if game.halfmove_clock >= 100 {
                    game.result = GameResult::Draw;
                    game.status = GameStatus::Finished;
                }
            }
        }
    }

    #[cfg(not(feature = "move-validation"))]
    {
        let _ = move_uci;
    }

    game.board_state = next_board;
    game.move_count = game
        .move_count
        .checked_add(1)
        .ok_or(GameErrorCode::ArithmeticOverflow)?;
    game.turn = game
        .turn
        .checked_add(1)
        .ok_or(GameErrorCode::ArithmeticOverflow)?;
    crate::lifecycle::clock::mark_activity(game, now);
    Ok(())
}
