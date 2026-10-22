use crate::constants::RECORD_RESULT_COST;
use crate::errors::GameErrorCode;
#[cfg(feature = "move-validation")]
use crate::state::GameResult;
use crate::state::{Game, GameStatus};
use anchor_lang::prelude::*;

#[cfg(feature = "move-validation")]
use crate::chess_logic_on_chain::nimzovich_engine::{
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
        let board = CompactBoard::from_bytes(&game.board_state);
        let mut position = board.to_on_chain_game();

        let outcome =
            validate_and_apply(&mut position, &move_uci).map_err(|_| GameErrorCode::InvalidMove)?;

        let expected = position.to_compact_board();
        let received = CompactBoard::from_bytes(&next_board);
        require!(expected == received, GameErrorCode::InvalidBoardState);

        let (from, to, _) = parse_uci(&move_uci).map_err(|_| GameErrorCode::InvalidMove)?;
        let from_index = usize::try_from(from).map_err(|_| GameErrorCode::InvalidMove)?;
        let to_index = usize::try_from(to).map_err(|_| GameErrorCode::InvalidMove)?;
        let piece = *board
            .squares
            .get(from_index)
            .ok_or(GameErrorCode::InvalidMove)?;
        let target = *board
            .squares
            .get(to_index)
            .ok_or(GameErrorCode::InvalidMove)?;
        let is_pawn_move = matches!(piece, 1 | -1);
        let is_capture = target != 0 || (is_pawn_move && (from % 8) != (to % 8));
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

    // Local builds may trust client boards without move-validation;
    // production requires it through the lib.rs compile guard.
    #[cfg(not(feature = "move-validation"))]
    {
        let _ = move_uci;
    }

    // Moving instead of accepting declines a pending offer, so it cannot be
    // accepted later in a position the offerer no longer considers drawn.
    // The offerer's own move leaves it standing for the opponent's reply.
    if game
        .draw_offered_by
        .is_some_and(|offerer| offerer != moving_player)
    {
        game.draw_offered_by = None;
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

#[cfg(all(test, feature = "move-validation"))]
mod tests {
    use super::*;
    use crate::state::{GameResult, GameType, MatchType};

    fn active_game() -> Game {
        Game {
            game_id: 1,
            white: Pubkey::new_unique(),
            black: Pubkey::new_unique(),
            status: GameStatus::Active,
            last_move_timestamp: 0,
            fees_advanced: 0,
            fee_payer: Pubkey::new_unique(),
            result: GameResult::None,
            board_state: CompactBoard::starting_position().to_bytes(),
            move_count: 0,
            halfmove_clock: 0,
            turn: 1,
            created_at: 0,
            updated_at: 0,
            wager_amount: 0,
            wager_token: None,
            game_type: GameType::PvP,
            match_type: MatchType::Free,
            country_fee: 0,
            base_time_seconds: 300,
            increment_seconds: 0,
            bump: 0,
            is_delegated: true,
            tournament_id: None,
            nonce: 0,
            draw_offered_by: None,
        }
    }

    fn play(game: &mut Game, player: Pubkey, uci: &str) -> Result<()> {
        let mut mv = [0u8; 5];
        mv[..uci.len()].copy_from_slice(uci.as_bytes());
        let mut position = CompactBoard::from_bytes(&game.board_state).to_on_chain_game();
        validate_and_apply(&mut position, &mv).map_err(|_| GameErrorCode::InvalidMove)?;
        let next_board = position.to_compact_board().to_bytes();
        let nonce = game.nonce + 1;
        apply_recorded_move(game, player, mv, next_board, nonce, None, 10)
    }

    #[test]
    fn opponent_move_declines_a_pending_draw_offer() -> anyhow::Result<()> {
        let mut game = active_game();
        let (white, black) = (game.white, game.black);
        play(&mut game, white, "e2e4").map_err(|_| anyhow::anyhow!("fixture move was rejected"))?;
        game.draw_offered_by = Some(white);

        play(&mut game, black, "e7e5").map_err(|_| anyhow::anyhow!("fixture move was rejected"))?;

        assert_eq!(game.draw_offered_by, None);
        assert!(
            crate::lifecycle::terminal::finish_by_draw_agreement(&mut game, black, 20).is_err(),
            "a declined offer cannot be accepted later"
        );
        Ok(())
    }

    #[test]
    fn offerers_own_move_keeps_the_offer_open_for_the_reply() -> anyhow::Result<()> {
        let mut game = active_game();
        let (white, black) = (game.white, game.black);
        game.draw_offered_by = Some(white);

        play(&mut game, white, "e2e4").map_err(|_| anyhow::anyhow!("fixture move was rejected"))?;

        assert_eq!(game.draw_offered_by, Some(white));
        crate::lifecycle::terminal::finish_by_draw_agreement(&mut game, black, 20)
            .map_err(|_| anyhow::anyhow!("fixture move was rejected"))?;
        assert_eq!(game.result, GameResult::Draw);
        Ok(())
    }
}
