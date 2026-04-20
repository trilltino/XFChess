//! Instruction for validating and recording a single state-transitioning chess move.

use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;

#[cfg(feature = "move-validation")]
use chess_logic_on_chain::shakmaty::{fen::Fen, uci::UciMove, Chess, Position, MoveList};

#[derive(Accounts)]
#[instruction(game_id: u64)]
pub struct RecordMove<'info> {
    #[account(mut, seeds = [GAME_SEED, &game_id.to_le_bytes()], bump)]
    pub game: Account<'info, Game>,
    #[account(mut, seeds = [MOVE_LOG_SEED, &game_id.to_le_bytes()], bump)]
    pub move_log: Account<'info, MoveLog>,
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
        constraint = Clock::get()?.unix_timestamp <= session_delegation.expires_at @ GameErrorCode::SessionExpired,
    )]
    pub session_delegation: Account<'info, SessionDelegation>,
}

pub fn handler(
    ctx: Context<RecordMove>,
    _game_id: u64,
    move_str: String,
    next_fen: String,
    nonce: u64,
    signature: Option<Vec<u8>>,
) -> Result<()> {
    // Capture before mutable borrows to avoid split-borrow issues inside cfg blocks
    let _moving_player = ctx.accounts.session_delegation.player;
    let game = &mut ctx.accounts.game;
    let move_log = &mut ctx.accounts.move_log;

    require!(
        game.status == GameStatus::Active,
        GameErrorCode::GameNotActive
    );

    // Replay Protection
    require!(nonce == move_log.nonce + 1, GameErrorCode::InvalidNonce);
    move_log.nonce = nonce;

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

        // 6. Auto-detect checkmate / stalemate
        // legal_moves() on the post-move position checks the side whose turn it now is.
        // If that side has no legal moves it is either checkmate (in check) or stalemate.
        let legal_moves: MoveList = next_pos.legal_moves();
        if legal_moves.is_empty() {
            if next_pos.checkers().any() {
                // Checkmate — the player who just moved wins
                game.result = GameResult::Winner(_moving_player);
                game.status = GameStatus::Finished;
                msg!("XFChess: CHECKMATE — {} wins", _moving_player);
            } else {
                // Stalemate — draw
                game.result = GameResult::Draw;
                game.status = GameStatus::Finished;
                msg!("XFChess: STALEMATE — draw");
            }
        }
    }

    game.fen = next_fen.clone();
    game.move_count += 1;
    game.turn += 1;
    let timestamp = Clock::get()?.unix_timestamp;
    game.updated_at = timestamp;

    // Emit move details to program logs for Explorer visibility
    msg!("XFChess: RecordMove");
    msg!("  move: {}", move_str);
    msg!("  fen: {}", next_fen);
    msg!("  nonce: {}", nonce);
    msg!("  timestamp: {}", timestamp);

    move_log.moves.push(move_str);
    move_log.timestamps.push(timestamp);
    if let Some(sig) = signature {
        move_log.player_signatures.push(sig);
    } else {
        move_log.player_signatures.push(Vec::new());
    }

    Ok(())
}
