//! Reusable lifecycle precondition checks.

use crate::errors::GameErrorCode;
use crate::state::{Game, GamePhase};
use anchor_lang::prelude::*;

/// Fails unless the game has already been undelegated back to the base
/// layer — base-layer-only operations (cancel, settlement) require this.
pub fn require_undelegated(game: &Game) -> Result<()> {
    require!(
        !game.is_delegated,
        GameErrorCode::SettlementRequiresUndelegated
    );
    Ok(())
}

/// Fails unless the game is currently delegated to the Ephemeral Rollup.
pub fn require_delegated(game: &Game) -> Result<()> {
    require!(game.is_delegated, GameErrorCode::GameNotDelegated);
    Ok(())
}

/// Fails unless the game's derived phase matches `expected`.
pub fn require_phase(game: &Game, expected: GamePhase) -> Result<()> {
    require!(
        game.phase()? == expected,
        GameErrorCode::InvalidLifecycleTransition
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{GameResult, GameStatus, GameType, MatchType};

    fn game(status: GameStatus, is_delegated: bool) -> Game {
        Game {
            game_id: 1,
            white: Pubkey::new_unique(),
            black: Pubkey::new_unique(),
            status,
            last_move_timestamp: 0,
            fees_advanced: 0,
            fee_payer: Pubkey::new_unique(),
            result: GameResult::None,
            board_state: [0; 68],
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
            base_time_seconds: 0,
            increment_seconds: 0,
            bump: 0,
            is_delegated,
            tournament_id: None,
            nonce: 0,
            draw_offered_by: None,
        }
    }

    #[test]
    fn require_undelegated_passes_when_not_delegated() {
        assert!(require_undelegated(&game(GameStatus::Active, false)).is_ok());
    }

    #[test]
    fn require_undelegated_fails_when_delegated() {
        assert!(require_undelegated(&game(GameStatus::Active, true)).is_err());
    }

    #[test]
    fn require_delegated_passes_when_delegated() {
        assert!(require_delegated(&game(GameStatus::Active, true)).is_ok());
    }

    #[test]
    fn require_delegated_fails_when_not_delegated() {
        assert!(require_delegated(&game(GameStatus::Active, false)).is_err());
    }

    #[test]
    fn require_phase_passes_on_matching_phase() {
        let g = game(GameStatus::Active, true);
        assert!(require_phase(&g, GamePhase::ActiveEr).is_ok());
    }

    #[test]
    fn require_phase_fails_on_mismatched_phase() {
        let g = game(GameStatus::Active, true);
        assert!(require_phase(&g, GamePhase::ActiveBase).is_err());
    }

    #[test]
    fn require_phase_propagates_unreachable_phase_error() {
        // (WaitingForOpponent, true) isn't a phase Game::phase() ever assigns —
        // require_phase must surface that error, not silently treat it as "no match".
        let g = game(GameStatus::WaitingForOpponent, true);
        assert!(require_phase(&g, GamePhase::WaitingBase).is_err());
    }
}
