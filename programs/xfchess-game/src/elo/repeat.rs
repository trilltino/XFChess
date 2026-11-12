//! Rating farming guard. Beating the same opponent again and again is the cheapest way to
//! inflate a rating, so each profile remembers its last rated opponent and how many rated
//! games in a row it has played against them. The weight of a result then fades:
//!
//! | consecutive games vs. the same opponent | share of the normal rating change |
//! |---|---|
//! | 1st to 3rd | 100% |
//! | 4th | 50% |
//! | 5th | 25% |
//! | 6th onwards | 0% |
//!
//! Playing someone else resets the count. This limits two-wallet win trading; it does not
//! stop a ring of three or more wallets, which needs per-pair history or off-chain analysis.
use crate::state::PlayerProfile;
use anchor_lang::prelude::Pubkey;

pub fn opponent_tag(opponent: &Pubkey) -> [u8; 8] {
    let mut tag = [0u8; 8];
    tag.copy_from_slice(&opponent.as_ref()[..8]);
    tag
}

pub fn repeat_weight_pct(streak: u8) -> u32 {
    match streak {
        0..=2 => 100,
        3 => 50,
        4 => 25,
        _ => 0,
    }
}

/// Records a rated game against `opponent` and returns the share of the rating change that
/// still applies to `profile`.
pub fn note_rated_opponent(profile: &mut PlayerProfile, opponent: &Pubkey) -> u32 {
    let tag = opponent_tag(opponent);
    if profile.last_rated_opponent == tag {
        profile.repeat_streak = profile.repeat_streak.saturating_add(1);
    } else {
        profile.last_rated_opponent = tag;
        profile.repeat_streak = 0;
    }
    repeat_weight_pct(profile.repeat_streak)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weight_fades_with_consecutive_games_and_stops_at_six() {
        let weights: Vec<u32> = (0..8).map(repeat_weight_pct).collect();
        assert_eq!(weights, [100, 100, 100, 50, 25, 0, 0, 0]);
    }

    #[test]
    fn a_new_opponent_resets_the_streak() {
        let (a, b) = (Pubkey::new_unique(), Pubkey::new_unique());
        let mut profile = PlayerProfile::default();
        let weights: Vec<u32> = (0..6).map(|_| note_rated_opponent(&mut profile, &a)).collect();
        assert_eq!(weights, [100, 100, 100, 50, 25, 0]);
        assert_eq!(note_rated_opponent(&mut profile, &b), 100);
        assert_eq!(profile.repeat_streak, 0);
        assert_eq!(note_rated_opponent(&mut profile, &a), 100, "going back starts over");
    }

    #[test]
    fn the_streak_saturates() {
        let a = Pubkey::new_unique();
        let mut profile = PlayerProfile::default();
        for _ in 0..400 {
            note_rated_opponent(&mut profile, &a);
        }
        assert_eq!(profile.repeat_streak, u8::MAX);
        assert_eq!(repeat_weight_pct(profile.repeat_streak), 0);
    }
}
