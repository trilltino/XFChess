//! Shared helpers for game creation and player/status guards.

use crate::constants::CREATE_GAME_COST;
use crate::errors::GameErrorCode;
use crate::state::{Game, GameResult, GameStatus, GameType, MatchType};
use anchor_lang::prelude::*;

pub struct InitGameArgs {
    pub game_id: u64,
    pub white: Pubkey,
    pub fee_payer: Pubkey,
    pub wager_amount: u64,
    pub match_type: MatchType,
    pub platform_fee: u64,
    pub base_time_seconds: u64,
    pub increment_seconds: u16,
    pub tournament_id: Option<u64>,
}

pub fn init_game_fields(game: &mut Game, args: InitGameArgs, now: i64, bump: u8) -> Result<()> {
    game.game_id = args.game_id;
    game.white = args.white;
    game.black = Pubkey::default();
    game.status = GameStatus::WaitingForOpponent;
    game.result = GameResult::None;
    #[cfg(feature = "move-validation")]
    {
        game.board_state =
            chess_logic_on_chain::nimzovich_engine::CompactBoard::starting_position().to_bytes();
    }
    #[cfg(not(feature = "move-validation"))]
    {
        game.board_state = [0; 68];
    }
    game.move_count = 0;
    game.halfmove_clock = 0;
    game.turn = 1;
    game.created_at = now;
    game.updated_at = now;
    game.last_move_timestamp = now;
    game.wager_amount = args.wager_amount;
    game.wager_token = None;
    game.game_type = GameType::PvP;
    game.match_type = args.match_type;
    game.country_fee = if args.match_type == MatchType::Free {
        0
    } else {
        args.platform_fee
    };
    game.base_time_seconds = args.base_time_seconds;
    game.increment_seconds = args.increment_seconds;
    game.bump = bump;
    game.fee_payer = args.fee_payer;
    game.fees_advanced = CREATE_GAME_COST;
    game.is_delegated = false;
    game.tournament_id = args.tournament_id;
    game.nonce = 0;
    Ok(())
}

pub fn require_player(game: &Game, player: Pubkey) -> Result<()> {
    require!(
        player == game.white || player == game.black,
        GameErrorCode::NotInGame
    );
    Ok(())
}

pub fn opponent_of(game: &Game, player: Pubkey) -> Result<Pubkey> {
    require_player(game, player)?;
    if player == game.white {
        Ok(game.black)
    } else {
        Ok(game.white)
    }
}
