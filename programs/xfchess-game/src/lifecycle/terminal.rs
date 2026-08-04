//! Terminal result transitions that mutate only the Game account.

use crate::errors::GameErrorCode;
use crate::lifecycle::clock;
use crate::state::{Game, GameResult, GameStatus};
use anchor_lang::prelude::*;

/// Ends an active game as a loss for `resigner`. Sets the result and status
/// only — does not touch `is_delegated` or move any funds (see settlement.rs).
pub fn finish_by_resign(game: &mut Game, resigner: Pubkey, now: i64) -> Result<()> {
    require!(
        game.status == GameStatus::Active,
        GameErrorCode::GameNotActive
    );
    require!(
        resigner == game.white || resigner == game.black,
        GameErrorCode::NotInGame
    );

    let winner = if resigner == game.white {
        game.black
    } else {
        game.white
    };

    game.result = GameResult::Winner(winner);
    game.status = GameStatus::Finished;
    clock::mark_terminal(game, now);
    Ok(())
}

/// Records `offerer`'s draw offer on an active game. Idempotent — re-offering
/// (including the other player "offering" after already receiving one, which
/// the client should instead treat as an implicit accept) just overwrites the
/// stored offerer.
pub fn record_draw_offer(game: &mut Game, offerer: Pubkey) -> Result<()> {
    require!(
        game.status == GameStatus::Active,
        GameErrorCode::GameNotActive
    );
    require!(
        offerer == game.white || offerer == game.black,
        GameErrorCode::NotInGame
    );

    game.draw_offered_by = Some(offerer);
    Ok(())
}

/// Ends an active game as a mutually agreed draw. `accepter` must be the
/// player who did *not* place the pending offer.
pub fn finish_by_draw_agreement(game: &mut Game, accepter: Pubkey, now: i64) -> Result<()> {
    require!(
        game.status == GameStatus::Active,
        GameErrorCode::GameNotActive
    );
    require!(
        accepter == game.white || accepter == game.black,
        GameErrorCode::NotInGame
    );
    let offerer = game
        .draw_offered_by
        .ok_or(GameErrorCode::NoDrawOfferPending)?;
    require!(
        accepter != offerer,
        GameErrorCode::CannotAcceptOwnDrawOffer
    );

    game.result = GameResult::Draw;
    game.status = GameStatus::Finished;
    game.draw_offered_by = None;
    clock::mark_terminal(game, now);
    Ok(())
}

/// Ends an active game as a loss for whoever's clock ran out, for the
/// permissionless `ClaimTimeout` instruction. Fails if the inactivity window
/// hasn't actually elapsed yet.
pub fn finish_by_timeout(game: &mut Game, now: i64) -> Result<()> {
    require!(
        game.status == GameStatus::Active,
        GameErrorCode::GameNotActive
    );

    let inactivity_window = clock::inactivity_window_seconds(game);
    require!(
        now - game.updated_at > inactivity_window,
        GameErrorCode::TimeoutNotExpired
    );

    let white_timed_out = game.turn % 2 == 1;
    let winner = if white_timed_out {
        game.black
    } else {
        game.white
    };

    game.result = GameResult::Winner(winner);
    game.status = GameStatus::Finished;
    clock::mark_terminal(game, now);
    Ok(())
}

/// The idempotent, crank-friendly variant of timeout resolution: returns
/// `Ok(false)` (rather than erroring) if the game isn't active or the clock
/// hasn't expired, so callers like `crank_ix::crank_time_check` can invoke it
/// unconditionally on every tick. Returns `Ok(true)` if it resolved the game.
pub fn finish_by_timeout_if_expired(game: &mut Game, now: i64) -> Result<bool> {
    if game.status != GameStatus::Active || game.base_time_seconds == 0 {
        return Ok(false);
    }

    let time_elapsed = (now - game.updated_at) as u64;
    let time_limit = game.base_time_seconds.saturating_mul(3);
    if time_elapsed <= time_limit {
        return Ok(false);
    }

    let white_timed_out = game.turn % 2 == 1;
    let winner = if white_timed_out {
        game.black
    } else {
        game.white
    };

    game.status = GameStatus::Finished;
    game.result = GameResult::Winner(winner);
    clock::mark_terminal(game, now);
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

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
            board_state: [0; 68],
            move_count: 0,
            halfmove_clock: 0,
            turn: 1,
            created_at: 0,
            updated_at: 0,
            wager_amount: 0,
            wager_token: None,
            game_type: crate::state::GameType::PvP,
            match_type: crate::state::MatchType::Free,
            country_fee: 0,
            base_time_seconds: 1,
            increment_seconds: 0,
            bump: 0,
            is_delegated: true,
            tournament_id: None,
            nonce: 0,
            draw_offered_by: None,
        }
    }

    #[test]
    fn resign_finishes_without_clearing_delegation() {
        let mut game = active_game();
        let white = game.white;
        let black = game.black;

        finish_by_resign(&mut game, white, 10).unwrap();

        assert_eq!(game.status, GameStatus::Finished);
        assert_eq!(game.result, GameResult::Winner(black));
        assert!(game.is_delegated);
        assert_eq!(game.updated_at, 10);
    }

    #[test]
    fn draw_offer_then_accept_by_other_player_finishes_as_draw() {
        let mut game = active_game();
        let white = game.white;
        let black = game.black;

        record_draw_offer(&mut game, white).unwrap();
        assert_eq!(game.draw_offered_by, Some(white));
        assert_eq!(game.status, GameStatus::Active, "offering alone doesn't end the game");

        finish_by_draw_agreement(&mut game, black, 10).unwrap();

        assert_eq!(game.status, GameStatus::Finished);
        assert_eq!(game.result, GameResult::Draw);
        assert_eq!(game.draw_offered_by, None);
        assert!(game.is_delegated);
        assert_eq!(game.updated_at, 10);
    }

    #[test]
    fn accept_draw_rejects_accepting_own_offer() {
        let mut game = active_game();
        let white = game.white;

        record_draw_offer(&mut game, white).unwrap();
        let err = finish_by_draw_agreement(&mut game, white, 10).unwrap_err();

        assert_eq!(err, GameErrorCode::CannotAcceptOwnDrawOffer.into());
        assert_eq!(game.status, GameStatus::Active);
    }

    #[test]
    fn accept_draw_rejects_when_no_offer_pending() {
        let mut game = active_game();
        let black = game.black;

        let err = finish_by_draw_agreement(&mut game, black, 10).unwrap_err();

        assert_eq!(err, GameErrorCode::NoDrawOfferPending.into());
    }

    #[test]
    fn offer_draw_rejects_non_participant() {
        let mut game = active_game();
        let stranger = Pubkey::new_unique();

        let err = record_draw_offer(&mut game, stranger).unwrap_err();

        assert_eq!(err, GameErrorCode::NotInGame.into());
    }

    #[test]
    fn timeout_finishes_without_clearing_delegation() {
        let mut game = active_game();
        let black = game.black;

        finish_by_timeout(&mut game, 10).unwrap();

        assert_eq!(game.status, GameStatus::Finished);
        assert_eq!(game.result, GameResult::Winner(black));
        assert!(game.is_delegated);
    }

    #[test]
    fn timeout_awards_white_when_black_flags() {
        // Mirror of `timeout_finishes_without_clearing_delegation`: turn is
        // even here (black to move / black's clock is the one that ran out),
        // locking in the other parity of the `white_timed_out` branch.
        let mut game = active_game();
        game.turn = 2;
        let white = game.white;

        finish_by_timeout(&mut game, 10).unwrap();

        assert_eq!(game.status, GameStatus::Finished);
        assert_eq!(game.result, GameResult::Winner(white));
    }

    #[test]
    fn timeout_rejects_before_inactivity_window_elapses() {
        let mut game = active_game(); // base_time_seconds: 1 -> window = 3s
        game.updated_at = 100;

        let err = finish_by_timeout(&mut game, 102).unwrap_err();

        assert_eq!(err, GameErrorCode::TimeoutNotExpired.into());
        assert_eq!(
            game.status,
            GameStatus::Active,
            "a rejected timeout claim must not mutate the game"
        );
    }

    /// docs/PRE_MAINNET_E2E_PLAN.md §1.3: `finish_by_timeout` (manual
    /// `ClaimTimeout`) and `finish_by_timeout_if_expired` (the crank's
    /// idempotent twin) are two independent implementations of the same
    /// payout decision. This locks them together so a future edit to one
    /// can't silently diverge from the other.
    #[test]
    fn crank_twin_matches_manual_claim_timeout_outcome() {
        for turn in [1u16, 2u16] {
            let mut manual = active_game();
            manual.turn = turn;
            let mut cranked = manual.clone();

            finish_by_timeout(&mut manual, 10).unwrap();
            let resolved = finish_by_timeout_if_expired(&mut cranked, 10).unwrap();

            assert!(resolved, "crank should resolve an expired game (turn={turn})");
            assert_eq!(cranked.status, manual.status);
            assert_eq!(cranked.result, manual.result);
        }
    }

    #[test]
    fn crank_twin_is_a_noop_before_inactivity_window_elapses() {
        let mut game = active_game();
        game.updated_at = 100;

        let resolved = finish_by_timeout_if_expired(&mut game, 102).unwrap();

        assert!(!resolved);
        assert_eq!(game.status, GameStatus::Active);
    }
}
