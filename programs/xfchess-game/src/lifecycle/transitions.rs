//! Small lifecycle transition helpers used by instruction adapters.

use crate::constants::{DELEGATE_COST, ER_SESSION_FEE_LAMPORTS, JOIN_GAME_COST, UNDELEGATE_COST};
use crate::errors::GameErrorCode;
use crate::state::{Game, GameStatus};
use anchor_lang::prelude::*;

/// Transitions a `WaitingForOpponent` game to `Active` with `joiner` as
/// black. Used by `game_ix::join`. Note: `game_ix::global_join` performs the
/// same transition inline rather than calling this helper — keep the two in
/// sync if this logic changes.
pub fn join_waiting_game(
    game: &mut Game,
    joiner: Pubkey,
    fee_payer: Pubkey,
    now: i64,
) -> Result<()> {
    require!(
        game.status == GameStatus::WaitingForOpponent,
        GameErrorCode::GameAlreadyFull
    );
    require!(game.white != joiner, GameErrorCode::CannotPlaySelf);
    require!(game.fee_payer == fee_payer, GameErrorCode::FeePayerMismatch);

    game.black = joiner;
    game.status = GameStatus::Active;
    game.fees_advanced = game
        .fees_advanced
        .checked_add(JOIN_GAME_COST)
        .ok_or(GameErrorCode::ArithmeticOverflow)?;
    game.last_move_timestamp = now;
    game.updated_at = now;
    Ok(())
}

/// Marks an active, not-yet-delegated game as delegated to the Ephemeral
/// Rollup and records the delegation fee advance. The single writer of
/// `is_delegated = true` — call this instead of setting the field directly.
pub fn mark_delegated(game: &mut Game) -> Result<()> {
    require!(
        game.status == GameStatus::Active,
        GameErrorCode::GameNotActive
    );
    require!(!game.is_delegated, GameErrorCode::GameAlreadyDelegated);
    game.fees_advanced = game
        .fees_advanced
        .checked_add(DELEGATE_COST)
        .ok_or(GameErrorCode::ArithmeticOverflow)?;
    game.is_delegated = true;
    Ok(())
}

/// Marks a delegated game as undelegated back to the base layer, and records
/// the reimbursable cost of that transition. The single writer of
/// `is_delegated = false` — call this instead of setting the field directly.
///
/// Accrues both `UNDELEGATE_COST` (the base-layer `undelegate_game` tx-fee
/// advance) and `ER_SESSION_FEE_LAMPORTS` (MagicBlock's ER-side session fee,
/// deducted from the session key off-chain and otherwise untracked) into
/// `fees_advanced`, so `finalize_game` reimburses the platform for the real
/// cost of every undelegate, not just create/join/delegate/record_move.
pub fn mark_undelegated(game: &mut Game) -> Result<()> {
    require!(game.is_delegated, GameErrorCode::GameNotDelegated);
    game.fees_advanced = game
        .fees_advanced
        .checked_add(UNDELEGATE_COST)
        .ok_or(GameErrorCode::ArithmeticOverflow)?
        .checked_add(ER_SESSION_FEE_LAMPORTS)
        .ok_or(GameErrorCode::ArithmeticOverflow)?;
    game.is_delegated = false;
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
    fn mark_delegated_accrues_delegate_cost() {
        let mut g = game(GameStatus::Active, false);
        mark_delegated(&mut g).unwrap();
        assert_eq!(g.fees_advanced, DELEGATE_COST);
        assert!(g.is_delegated);
    }

    #[test]
    fn mark_undelegated_accrues_undelegate_and_session_fee() {
        let mut g = game(GameStatus::Active, true);
        g.fees_advanced = DELEGATE_COST;
        mark_undelegated(&mut g).unwrap();
        assert_eq!(
            g.fees_advanced,
            DELEGATE_COST + UNDELEGATE_COST + ER_SESSION_FEE_LAMPORTS
        );
        assert!(!g.is_delegated);
    }

    #[test]
    fn full_lifecycle_fees_advanced_matches_sum_of_all_flat_costs() {
        use crate::constants::{CREATE_GAME_COST, RECORD_RESULT_COST};

        let mut g = game(GameStatus::WaitingForOpponent, false);
        g.fees_advanced = CREATE_GAME_COST;
        let fee_payer = g.fee_payer;

        join_waiting_game(&mut g, Pubkey::new_unique(), fee_payer, 0).unwrap();
        mark_delegated(&mut g).unwrap();

        const MOVES: u64 = 4;
        for _ in 0..MOVES {
            g.fees_advanced = g.fees_advanced.checked_add(RECORD_RESULT_COST).unwrap();
        }

        mark_undelegated(&mut g).unwrap();

        assert_eq!(
            g.fees_advanced,
            CREATE_GAME_COST
                + JOIN_GAME_COST
                + DELEGATE_COST
                + MOVES * RECORD_RESULT_COST
                + UNDELEGATE_COST
                + ER_SESSION_FEE_LAMPORTS
        );
    }

    #[test]
    fn mark_undelegated_rejects_non_delegated_game() {
        let mut g = game(GameStatus::Active, false);
        assert!(mark_undelegated(&mut g).is_err());
    }
}
