//! Shared dispute resolution validation and state transitions.

use crate::errors::GameErrorCode;
use crate::state::{DisputeRecord, DisputeStatus, Game, GameResult, GameStatus};
use anchor_lang::prelude::*;

/// Cap on `DisputeGame`'s `reason` and `ResolveDispute`'s `resolution` text.
pub const MAX_DISPUTE_TEXT_LEN: usize = 200;

/// Rejects free-text fields longer than `MAX_DISPUTE_TEXT_LEN`.
pub fn require_text_fits(text: &str) -> Result<()> {
    require!(
        text.len() <= MAX_DISPUTE_TEXT_LEN,
        GameErrorCode::InvalidArgument
    );
    Ok(())
}

/// Turns an authority's `winner` choice into a `GameResult`, rejecting a
/// winner that isn't actually one of the game's two players.
pub fn validate_resolution(game: &Game, winner: Option<Pubkey>) -> Result<GameResult> {
    match winner {
        Some(winner_key) => {
            require!(
                winner_key == game.white || winner_key == game.black,
                GameErrorCode::InvalidWinner
            );
            Ok(GameResult::Winner(winner_key))
        }
        None => Ok(GameResult::Draw),
    }
}

/// Writes the platform authority's ruling onto both the dispute and the
/// game: marks the dispute `Resolved` with its resolution text, and the game
/// `Settled` with the given result. Used by `resolve.rs`'s authority-ruled
/// path only — `claim_stale_dispute.rs`'s TTL auto-resolution sets its own
/// `Dismissed` status inline instead.
pub fn apply_resolution(
    game: &mut Game,
    dispute: &mut DisputeRecord,
    result: GameResult,
    resolution: String,
    resolved_by: Pubkey,
    now: i64,
) -> Result<()> {
    require_text_fits(&resolution)?;
    dispute.status = DisputeStatus::Resolved;
    dispute.resolution = resolution;
    dispute.resolved_at = Some(now);
    dispute.resolved_by = Some(resolved_by);
    game.result = result;
    game.status = GameStatus::Settled;
    game.updated_at = now;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{GameType, MatchType};

    fn game(white: Pubkey, black: Pubkey) -> Game {
        Game {
            game_id: 1,
            white,
            black,
            status: GameStatus::Disputed,
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
            is_delegated: false,
            tournament_id: None,
            nonce: 0,
            draw_offered_by: None,
        }
    }

    fn dispute(game_id: u64, challenger: Pubkey) -> DisputeRecord {
        DisputeRecord {
            game_id,
            challenger,
            reason: "suspected engine use".to_string(),
            evidence_hash: [0; 32],
            status: DisputeStatus::Pending,
            resolved_by: None,
            resolution: String::new(),
            created_at: 0,
            expires_at: 1000,
            resolved_at: None,
            bond_amount: 0,
            bump: 0,
        }
    }

    #[test]
    fn require_text_fits_accepts_the_exact_cap() {
        let text: String = "a".repeat(MAX_DISPUTE_TEXT_LEN);
        assert!(require_text_fits(&text).is_ok());
    }

    #[test]
    fn require_text_fits_rejects_one_over_the_cap() {
        let text: String = "a".repeat(MAX_DISPUTE_TEXT_LEN + 1);
        assert!(require_text_fits(&text).is_err());
    }

    #[test]
    fn validate_resolution_none_is_a_draw() {
        let g = game(Pubkey::new_unique(), Pubkey::new_unique());
        assert_eq!(validate_resolution(&g, None).unwrap(), GameResult::Draw);
    }

    #[test]
    fn validate_resolution_accepts_a_real_player_as_winner() {
        let white = Pubkey::new_unique();
        let black = Pubkey::new_unique();
        let g = game(white, black);
        assert_eq!(
            validate_resolution(&g, Some(black)).unwrap(),
            GameResult::Winner(black)
        );
    }

    #[test]
    fn validate_resolution_rejects_a_non_player_winner() {
        let g = game(Pubkey::new_unique(), Pubkey::new_unique());
        assert!(validate_resolution(&g, Some(Pubkey::new_unique())).is_err());
    }

    #[test]
    fn apply_resolution_updates_both_dispute_and_game() {
        let white = Pubkey::new_unique();
        let black = Pubkey::new_unique();
        let mut g = game(white, black);
        let mut d = dispute(g.game_id, white);
        let authority = Pubkey::new_unique();

        apply_resolution(
            &mut g,
            &mut d,
            GameResult::Winner(black),
            "engine use confirmed".to_string(),
            authority,
            500,
        )
        .unwrap();

        assert_eq!(d.status, DisputeStatus::Resolved);
        assert_eq!(d.resolution, "engine use confirmed");
        assert_eq!(d.resolved_at, Some(500));
        assert_eq!(d.resolved_by, Some(authority));
        assert_eq!(g.result, GameResult::Winner(black));
        assert_eq!(g.status, GameStatus::Settled);
        assert_eq!(g.updated_at, 500);
    }

    #[test]
    fn apply_resolution_rejects_oversized_text_before_mutating() {
        let white = Pubkey::new_unique();
        let black = Pubkey::new_unique();
        let mut g = game(white, black);
        let mut d = dispute(g.game_id, white);
        let oversized = "a".repeat(MAX_DISPUTE_TEXT_LEN + 1);

        let err = apply_resolution(
            &mut g,
            &mut d,
            GameResult::Draw,
            oversized,
            Pubkey::new_unique(),
            500,
        );

        assert!(err.is_err());
        assert_eq!(
            d.status,
            DisputeStatus::Pending,
            "must not mutate on failure"
        );
        assert_eq!(g.status, GameStatus::Disputed, "must not mutate on failure");
    }
}
