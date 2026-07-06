use anchor_lang::prelude::*;
use xfchess_game::state::{MatchStatus, TournamentMatch};
use xfchess_game::tournament_ix::matches::guards;

fn tournament_match(white: Pubkey, black: Pubkey) -> TournamentMatch {
    TournamentMatch {
        tournament_id: 1,
        match_index: 0,
        round: 0,
        player_white: Some(white),
        player_black: Some(black),
        winner: None,
        game_pda: None,
        game_id: None,
        status: MatchStatus::Active,
        next_match_for_winner: None,
        next_match_slot: 0,
        started_at: None,
        completed_at: None,
        bump: 0,
    }
}

#[test]
fn match_participant_guard_rejects_outsiders_and_same_player() {
    let white = Pubkey::new_unique();
    let black = Pubkey::new_unique();
    let tm = tournament_match(white, black);

    assert!(guards::require_match_participants(&tm, white, white).is_err());
    assert!(guards::require_match_participants(&tm, white, Pubkey::new_unique()).is_err());
    assert!(guards::require_match_participants(&tm, white, black).is_ok());
    assert!(guards::require_match_participants(&tm, black, white).is_ok());
}
