use crate::errors::GameErrorCode;
use crate::state::PlayerProfile;
use anchor_lang::prelude::*;
#[cfg(test)]
use anyhow::Context as _;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RatingBucket {
    Bullet,
    Blitz,
    Rapid,
    Classical,
}

pub fn bucket_for_time_control(base_time_seconds: u64) -> RatingBucket {
    match base_time_seconds {
        0 => RatingBucket::Classical,
        1..=179 => RatingBucket::Bullet,
        180..=599 => RatingBucket::Blitz,
        600..=1499 => RatingBucket::Rapid,
        _ => RatingBucket::Classical,
    }
}

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

/// A profile's display rating for a time control. A bucket the player has not
/// played yet (0.0, or anything non-finite) counts as the initial rating, the
/// same value `rating_field_mut` seeds on first use.
pub fn display_rating(profile: &PlayerProfile, base_time_seconds: u64) -> u32 {
    let rating = match bucket_for_time_control(base_time_seconds) {
        RatingBucket::Bullet => profile.elo_bullet,
        RatingBucket::Blitz => profile.elo_blitz,
        RatingBucket::Rapid => profile.elo_rapid,
        RatingBucket::Classical => profile.elo_rating,
    };
    if rating.is_finite() && rating > 0.0 {
        centiscale_to_display(rating)
    } else {
        INITIAL_ELO
    }
}

pub const RATING_SCALE: u32 = 100;
pub const INITIAL_ELO: u32 = 1200;
pub const INITIAL_ELO_CENTISCALE: u32 = INITIAL_ELO * RATING_SCALE;
pub const MIN_EXTERNAL_ELO: u32 = 100;
pub const MAX_EXTERNAL_ELO: u32 = 4000;

pub fn external_to_centiscale(rating: u32) -> Result<u32> {
    validate_external_rating(rating)?;
    rating
        .checked_mul(RATING_SCALE)
        .ok_or_else(|| GameErrorCode::ArithmeticOverflow.into())
}

pub fn centiscale_to_display(rating: f64) -> u32 {
    (rating / RATING_SCALE as f64).round() as u32
}

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
    fn external_to_centiscale_scales_by_100() -> anyhow::Result<()> {
        assert_eq!(
            external_to_centiscale(1500).context("required operation value")?,
            150000
        );
        assert_eq!(
            external_to_centiscale(MIN_EXTERNAL_ELO).context("required operation value")?,
            10000
        );
        Ok(())
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
        assert_eq!(bucket_for_time_control(0), RatingBucket::Classical);
        assert_eq!(bucket_for_time_control(15), RatingBucket::Bullet);
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
            elo_rating: 130000.0,
            ..Default::default()
        };

        assert_eq!(profile.elo_bullet, 0.0);
        assert_eq!(
            *rating_field_mut(&mut profile, RatingBucket::Bullet),
            INITIAL_ELO_CENTISCALE as f64
        );
        assert_eq!(profile.elo_bullet, INITIAL_ELO_CENTISCALE as f64);

        assert_eq!(
            *rating_field_mut(&mut profile, RatingBucket::Classical),
            130000.0
        );
    }

    #[test]
    fn display_rating_reads_the_time_control_bucket() {
        let profile = PlayerProfile {
            elo_rating: 180_000.0,
            elo_blitz: 150_049.0,
            ..Default::default()
        };
        assert_eq!(display_rating(&profile, 300), 1500);
        assert_eq!(display_rating(&profile, 0), 1800);
        // Unplayed buckets and corrupt values fall back to the initial rating.
        assert_eq!(display_rating(&profile, 60), INITIAL_ELO);
        let corrupt = PlayerProfile {
            elo_rapid: f64::NAN,
            ..Default::default()
        };
        assert_eq!(display_rating(&corrupt, 600), INITIAL_ELO);
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

    fn rated_profile() -> PlayerProfile {
        PlayerProfile {
            elo_bullet: 111_100.0,
            elo_blitz: 222_200.0,
            elo_rapid: 333_300.0,
            elo_rating: 444_400.0,
            ..Default::default()
        }
    }

    #[test]
    fn display_rating_reads_the_bucket_of_the_time_control() {
        let profile = rated_profile();
        for (base_time_seconds, expected) in [
            (60, 1_111),
            (300, 2_222),
            (900, 3_333),
            (1_800, 4_444),
            (0, 4_444),
        ] {
            assert_eq!(
                display_rating(&profile, base_time_seconds),
                expected,
                "base time {base_time_seconds}"
            );
        }
    }

    #[test]
    fn display_rating_rounds_to_the_nearest_elo() {
        let profile = PlayerProfile {
            elo_blitz: 150_049.0,
            elo_rapid: 150_050.0,
            ..Default::default()
        };
        assert_eq!(display_rating(&profile, 300), 1_500);
        assert_eq!(display_rating(&profile, 900), 1_501);
    }

    #[test]
    fn display_rating_treats_an_untouched_bucket_as_the_initial_rating() {
        let profile = PlayerProfile::default();
        for base_time_seconds in [0, 60, 300, 900, 1_800] {
            assert_eq!(display_rating(&profile, base_time_seconds), INITIAL_ELO);
        }
    }

    #[test]
    fn display_rating_counts_unusable_ratings_as_the_initial_rating() {
        for stored in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0] {
            let profile = PlayerProfile {
                elo_blitz: stored,
                ..Default::default()
            };
            assert_eq!(
                display_rating(&profile, 300),
                INITIAL_ELO,
                "stored {stored}"
            );
        }
    }
}
