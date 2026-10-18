use crate::errors::GameErrorCode;
use crate::lifecycle::clock::timeout_window_seconds;
use crate::state::{Game, GamePhase, GameStatus};
use anchor_lang::prelude::*;

pub fn require_undelegated(game: &Game) -> Result<()> {
    require!(
        !game.is_delegated,
        GameErrorCode::SettlementRequiresUndelegated
    );
    Ok(())
}

pub fn require_delegated(game: &Game) -> Result<()> {
    require!(game.is_delegated, GameErrorCode::GameNotDelegated);
    Ok(())
}

/// The forced-undelegation escape hatch is for an ER that has stopped serving
/// a game, and its timeout path wipes the Game account. A request therefore
/// needs the last committed state to show the ER failing: a terminal result
/// that was never returned to base, or no move for longer than the window in
/// which anyone could end the game on time. A live game cannot be targeted.
pub fn require_stuck_delegation(game: &Game, now: i64) -> Result<()> {
    let terminal = matches!(game.status, GameStatus::Finished | GameStatus::Cancelled);
    let idle = now.saturating_sub(game.updated_at) > timeout_window_seconds(game);
    require!(terminal || idle, GameErrorCode::GameNotStuckDelegation);
    Ok(())
}

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
    use crate::state::{GameResult, GameType, MatchType};

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
    fn live_delegated_game_cannot_be_force_undelegated() {
        let mut g = game(GameStatus::Active, true);
        g.move_count = 10;
        g.updated_at = 1_000;
        assert!(require_stuck_delegation(&g, 1_010).is_err());
        assert!(require_stuck_delegation(&g, 1_000 + 86_400 + 1).is_ok());
    }

    #[test]
    fn unreturned_terminal_game_can_be_force_undelegated() {
        for status in [GameStatus::Finished, GameStatus::Cancelled] {
            let mut g = game(status, true);
            g.updated_at = 1_000;
            assert!(require_stuck_delegation(&g, 1_001).is_ok());
        }
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
        let g = game(GameStatus::WaitingForOpponent, true);
        assert!(require_phase(&g, GamePhase::WaitingBase).is_err());
    }
}
