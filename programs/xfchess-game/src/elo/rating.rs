//! Rating unit conversions and per-time-control bucketing.
//!
//! On-chain `PlayerProfile.elo_rating` and linked external ratings are stored
//! in centiscale: 1200 Elo is stored as 120000.

use crate::errors::GameErrorCode;
use crate::state::PlayerProfile;
use anchor_lang::prelude::*;

/// Which of `PlayerProfile`'s four rating fields a game's outcome updates.
/// Mirrors `TimeCategory` in the game client's `src/game/time_control.rs`,
/// collapsing `UltraBullet` into `Bullet` and `Unlimited` into `Classical` —
/// the on-chain profile only tracks four buckets, not six.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RatingBucket {
    Bullet,
    Blitz,
    Rapid,
    Classical,
}

/// Buckets a game's base time control into a rating category. Thresholds
/// match the client's `TimeControl::category()` exactly (seconds of base
/// time per player; increment isn't a factor, same as the client).
pub fn bucket_for_time_control(base_time_seconds: u64) -> RatingBucket {
    match base_time_seconds {
        0 => RatingBucket::Classical, // Unlimited/no-clock games
        1..=179 => RatingBucket::Bullet,
        180..=599 => RatingBucket::Blitz,
        600..=1499 => RatingBucket::Rapid,
        _ => RatingBucket::Classical,
    }
}

/// Mutable access to the rating field a bucket maps to, lazily seeding it to
/// `INITIAL_ELO_CENTISCALE` on first touch (mirrors the pre-existing
/// lazy-init for `elo_rating` in `account_ix::profile_init`) — a player's
/// first-ever Bullet game shouldn't start from 0.0 just because they already
/// have a Blitz rating.
pub fn rating_field_mut(profile: &mut PlayerProfile, bucket: RatingBucket) -> &mut f64 {
    let field = match bucket {
        RatingBucket::Bullet => &mut profile.elo_bullet,
        RatingBucket::Blitz => &mut profile.elo_blitz,
        RatingBucket::Rapid => &mut profile.elo_rapid,
        RatingBucket::Classical => &mut profile.elo_rating,
    };
    if *field == 0.0 {
        *field = INITIAL_ELO_CENTISCALE as f64;
    }
    field
}

pub const RATING_SCALE: u32 = 100;
pub const INITIAL_ELO: u32 = 1200;
pub const INITIAL_ELO_CENTISCALE: u32 = INITIAL_ELO * RATING_SCALE;
pub const MIN_EXTERNAL_ELO: u32 = 100;
pub const MAX_EXTERNAL_ELO: u32 = 4000;

/// Converts a display-scale external rating (e.g. a Lichess rating) to
/// centiscale for on-chain storage, after validating it's in range.
pub fn external_to_centiscale(rating: u32) -> Result<u32> {
    validate_external_rating(rating)?;
    rating
        .checked_mul(RATING_SCALE)
        .ok_or_else(|| GameErrorCode::ArithmeticOverflow.into())
}

/// Converts a centiscale rating back to display scale (rounded), e.g. for UI rendering.
pub fn centiscale_to_display(rating: f64) -> u32 {
    (rating / RATING_SCALE as f64).round() as u32
}

/// Rejects external ratings outside `MIN_EXTERNAL_ELO..=MAX_EXTERNAL_ELO`.
pub fn validate_external_rating(rating: u32) -> Result<()> {
    require!(
        (MIN_EXTERNAL_ELO..=MAX_EXTERNAL_ELO).contains(&rating),
        GameErrorCode::EloOutOfRange
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_accepts_range_boundaries() {
        assert!(validate_external_rating(MIN_EXTERNAL_ELO).is_ok());
        assert!(validate_external_rating(MAX_EXTERNAL_ELO).is_ok());
    }

    #[test]
    fn validate_rejects_just_outside_boundaries() {
        assert!(validate_external_rating(MIN_EXTERNAL_ELO - 1).is_err());
        assert!(validate_external_rating(MAX_EXTERNAL_ELO + 1).is_err());
    }

    #[test]
    fn external_to_centiscale_scales_by_100() {
        assert_eq!(external_to_centiscale(1500).unwrap(), 150000);
        assert_eq!(external_to_centiscale(MIN_EXTERNAL_ELO).unwrap(), 10000);
    }

    #[test]
    fn external_to_centiscale_rejects_out_of_range_before_scaling() {
        assert!(external_to_centiscale(MAX_EXTERNAL_ELO + 1).is_err());
    }

    #[test]
    fn centiscale_to_display_rounds_to_nearest_elo() {
        assert_eq!(centiscale_to_display(150000.0), 1500);
        assert_eq!(centiscale_to_display(150049.0), 1500);
        assert_eq!(centiscale_to_display(150050.0), 1501);
    }

    #[test]
    fn initial_elo_constant_round_trips() {
        assert_eq!(INITIAL_ELO_CENTISCALE, INITIAL_ELO * RATING_SCALE);
        assert_eq!(
            centiscale_to_display(INITIAL_ELO_CENTISCALE as f64),
            INITIAL_ELO
        );
    }

    #[test]
    fn bucket_for_time_control_matches_client_thresholds() {
        // Mirrors src/game/time_control.rs's TimeControl::category(), with
        // UltraBullet folded into Bullet and Unlimited folded into Classical.
        assert_eq!(bucket_for_time_control(0), RatingBucket::Classical); // Unlimited
        assert_eq!(bucket_for_time_control(15), RatingBucket::Bullet); // UltraBullet
        assert_eq!(bucket_for_time_control(60), RatingBucket::Bullet);
        assert_eq!(bucket_for_time_control(179), RatingBucket::Bullet);
        assert_eq!(bucket_for_time_control(180), RatingBucket::Blitz);
        assert_eq!(bucket_for_time_control(300), RatingBucket::Blitz);
        assert_eq!(bucket_for_time_control(599), RatingBucket::Blitz);
        assert_eq!(bucket_for_time_control(600), RatingBucket::Rapid);
        assert_eq!(bucket_for_time_control(1499), RatingBucket::Rapid);
        assert_eq!(bucket_for_time_control(1500), RatingBucket::Classical);
        assert_eq!(bucket_for_time_control(1800), RatingBucket::Classical);
    }

    #[test]
    fn rating_field_mut_lazily_seeds_on_first_touch() {
        let mut profile = PlayerProfile {
            elo_rating: 130000.0, // Classical already played
            ..Default::default()
        };

        // Untouched bucket (Bullet) starts at 0.0 and gets seeded on access.
        assert_eq!(profile.elo_bullet, 0.0);
        assert_eq!(
            *rating_field_mut(&mut profile, RatingBucket::Bullet),
            INITIAL_ELO_CENTISCALE as f64
        );
        assert_eq!(profile.elo_bullet, INITIAL_ELO_CENTISCALE as f64);

        // Already-seeded bucket (Classical) is returned as-is, not reset.
        assert_eq!(
            *rating_field_mut(&mut profile, RatingBucket::Classical),
            130000.0
        );
    }

    #[test]
    fn rating_field_mut_maps_each_bucket_to_its_own_field() {
        let mut profile = PlayerProfile::default();
        *rating_field_mut(&mut profile, RatingBucket::Bullet) = 111111.0;
        *rating_field_mut(&mut profile, RatingBucket::Blitz) = 222222.0;
        *rating_field_mut(&mut profile, RatingBucket::Rapid) = 333333.0;
        *rating_field_mut(&mut profile, RatingBucket::Classical) = 444444.0;

        assert_eq!(profile.elo_bullet, 111111.0);
        assert_eq!(profile.elo_blitz, 222222.0);
        assert_eq!(profile.elo_rapid, 333333.0);
        assert_eq!(profile.elo_rating, 444444.0);
    }
}
