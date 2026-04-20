use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;

#[cfg(feature = "move-validation")]
use chess_logic_on_chain::shakmaty::{fen::Fen, uci::UciMove, Chess, Position};

#[derive(Accounts)]
#[instruction(game_id: u64)]
pub struct RecordMove<'info> {
    #[account(mut, seeds = [GAME_SEED, &game_id.to_le_bytes()], bump)]
    pub game: Account<'info, Game>,
    #[account(mut, seeds = [MOVE_LOG_SEED, &game_id.to_le_bytes()], bump)]
    pub move_log: Account<'info, MoveLog>,
    pub player: Signer<'info>,
}

pub fn handler(
    ctx: Context<RecordMove>,
    _game_id: u64,
    move_str: String,
    next_fen: String,
) -> Result<()> {
    let game = &mut ctx.accounts.game;
    let move_log = &mut ctx.accounts.move_log;

    require!(
        game.status == GameStatus::Active,
        GameErrorCode::GameNotActive
    );

    let player = ctx.accounts.player.key();

    // Turn and Identity Validation
    if game.turn % 2 != 0 {
        // White's turn
        require!(player == game.white, GameErrorCode::NotPlayerTurn);
    } else {
        // Black's turn (or AI's turn)
        if game.game_type == GameType::PvAI {
            require!(
                player == crate::constants::ai_authority::ID,
                GameErrorCode::NotPlayerTurn
            );
        } else {
            require!(player == game.black, GameErrorCode::NotPlayerTurn);
        }
    }

    // --- ON-CHAIN CHESS VALIDATION ---
    #[cfg(feature = "move-validation")]
    {
        // 1. Parse current board state from stored FEN
        let setup =
            Fen::from_ascii(game.fen.as_bytes()).map_err(|_| GameErrorCode::InvalidBoardState)?;
        let pos: Chess = setup
            .into_position(chess_logic_on_chain::shakmaty::CastlingMode::Standard)
            .map_err(|_| GameErrorCode::InvalidBoardState)?;

        // 2. Parse incoming UCI move string
        let uci = UciMove::from_ascii(move_str.as_bytes()).map_err(|_| GameErrorCode::InvalidMove)?;
        let m = uci.to_move(&pos).map_err(|_| GameErrorCode::InvalidMove)?;

        // 3. Check move legality
        require!(pos.is_legal(&m), GameErrorCode::InvalidMove);

        // 4. Apply the legal move
        let mut next_pos = pos.clone();
        next_pos.play_unchecked(&m);

        // 5. Verify the client's provided next_fen perfectly matches the applied move consequence
        let next_setup =
            Fen::from_ascii(next_fen.as_bytes()).map_err(|_| GameErrorCode::InvalidBoardState)?;
        let client_next_pos: Chess = next_setup
            .into_position(chess_logic_on_chain::shakmaty::CastlingMode::Standard)
            .map_err(|_| GameErrorCode::InvalidBoardState)?;

        require!(
            next_pos == client_next_pos,
            GameErrorCode::InvalidBoardState
        );
    }

    game.fen = next_fen;
    game.move_count += 1;
    game.turn += 1;
    game.updated_at = Clock::get()?.unix_timestamp;

    move_log.moves.push(move_str);

    Ok(())
}
