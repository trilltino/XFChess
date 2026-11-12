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

pub fn rating_field_mut(profile: &mut PlayerProfile, bucket: RatingBucket) -> &mut u32 {
    let field = match bucket {
        RatingBucket::Bullet => &mut profile.elo_bullet,
        RatingBucket::Blitz => &mut profile.elo_blitz,
        RatingBucket::Rapid => &mut profile.elo_rapid,
        RatingBucket::Classical => &mut profile.elo_rating,
    };
    if *field == 0 {
        *field = INITIAL_ELO_CENTISCALE;
    }
    field
}

/// Rated games played in a time control. A rating is provisional until this reaches
/// `PROVISIONAL_GAMES`. The counter saturates at 255.
pub fn games_field_mut(profile: &mut PlayerProfile, bucket: RatingBucket) -> &mut u8 {
    match bucket {
        RatingBucket::Bullet => &mut profile.games_bullet,
        RatingBucket::Blitz => &mut profile.games_blitz,
        RatingBucket::Rapid => &mut profile.games_rapid,
        RatingBucket::Classical => &mut profile.games_classical,
    }
}

/// A profile display rating for a time control. A bucket the player has not played yet (0)
/// counts as the initial rating, the same value `rating_field_mut` seeds on first use.
pub fn display_rating(profile: &PlayerProfile, base_time_seconds: u64) -> u32 {
    let rating = match bucket_for_time_control(base_time_seconds) {
        RatingBucket::Bullet => profile.elo_bullet,
        RatingBucket::Blitz => profile.elo_blitz,
        RatingBucket::Rapid => profile.elo_rapid,
        RatingBucket::Classical => profile.elo_rating,
    };
    if rating > 0 {
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

/// Nearest whole rating point.
pub fn centiscale_to_display(rating: u32) -> u32 {
    rating.saturating_add(RATING_SCALE / 2) / RATING_SCALE
}

pub fn validate_external_rating(rating: u32) -> Result<()> {
    require!(
        (MIN_EXTERNAL_ELO..=MAX_EXTERNAL_ELO).contains(&rating),
        GameErrorCode::EloOutOfRange
    );
    Ok(())
}

/// Lowest and highest rating a linked external account can seed, in centiscale (800-2200).
pub const SEED_MIN: u32 = 80_000;
pub const SEED_MAX: u32 = 220_000;

/// Starting rating for a player who links an external account: three quarters of the way
/// from the default to the external rating, kept within the seed range. External pools run
/// on a different scale, so this is a first guess that the provisional K then corrects.
pub fn seed_rating_from_external(external_centiscale: u32) -> u32 {
    let initial = i64::from(INITIAL_ELO_CENTISCALE);
    let seeded = initial + (i64::from(external_centiscale) - initial) * 3 / 4;
    seeded.clamp(i64::from(SEED_MIN), i64::from(SEED_MAX)) as u32
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
        assert_eq!(centiscale_to_display(150000), 1500);
        assert_eq!(centiscale_to_display(150049), 1500);
        assert_eq!(centiscale_to_display(150050), 1501);
        assert_eq!(centiscale_to_display(u32::MAX), u32::MAX / RATING_SCALE);
    }

    #[test]
    fn initial_elo_constant_round_trips() {
        assert_eq!(INITIAL_ELO_CENTISCALE, INITIAL_ELO * RATING_SCALE);
        assert_eq!(centiscale_to_display(INITIAL_ELO_CENTISCALE), INITIAL_ELO);
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
            elo_rating: 130000,
            ..Default::default()
        };

        assert_eq!(profile.elo_bullet, 0);
        assert_eq!(
            *rating_field_mut(&mut profile, RatingBucket::Bullet),
            INITIAL_ELO_CENTISCALE
        );
        assert_eq!(profile.elo_bullet, INITIAL_ELO_CENTISCALE);

        assert_eq!(
            *rating_field_mut(&mut profile, RatingBucket::Classical),
            130000
        );
    }

    #[test]
    fn rating_and_game_fields_map_each_bucket_to_its_own_field() {
        let mut profile = PlayerProfile::default();
        *rating_field_mut(&mut profile, RatingBucket::Bullet) = 111111;
        *rating_field_mut(&mut profile, RatingBucket::Blitz) = 222222;
        *rating_field_mut(&mut profile, RatingBucket::Rapid) = 333333;
        *rating_field_mut(&mut profile, RatingBucket::Classical) = 444444;
        *games_field_mut(&mut profile, RatingBucket::Bullet) = 1;
        *games_field_mut(&mut profile, RatingBucket::Blitz) = 2;
        *games_field_mut(&mut profile, RatingBucket::Rapid) = 3;
        *games_field_mut(&mut profile, RatingBucket::Classical) = 4;

        assert_eq!(profile.elo_bullet, 111111);
        assert_eq!(profile.elo_blitz, 222222);
        assert_eq!(profile.elo_rapid, 333333);
        assert_eq!(profile.elo_rating, 444444);
        assert_eq!(
            [
                profile.games_bullet,
                profile.games_blitz,
                profile.games_rapid,
                profile.games_classical
            ],
            [1, 2, 3, 4]
        );
    }

    fn rated_profile() -> PlayerProfile {
        PlayerProfile {
            elo_bullet: 111_100,
            elo_blitz: 222_200,
            elo_rapid: 333_300,
            elo_rating: 444_400,
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
            elo_blitz: 150_049,
            elo_rapid: 150_050,
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
    fn external_seeds_move_three_quarters_of_the_way_and_stay_in_range() {
        assert_eq!(seed_rating_from_external(120_000), 120_000);
        assert_eq!(seed_rating_from_external(160_000), 150_000);
        assert_eq!(seed_rating_from_external(80_000), 90_000);
        assert_eq!(
            seed_rating_from_external(MIN_EXTERNAL_ELO * RATING_SCALE),
            SEED_MIN
        );
        assert_eq!(
            seed_rating_from_external(MAX_EXTERNAL_ELO * RATING_SCALE),
            SEED_MAX
        );
    }
}
