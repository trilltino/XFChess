//! Rating unit conversions.
//!
//! On-chain `PlayerProfile.elo_rating` and linked external ratings are stored
//! in centiscale: 1200 Elo is stored as 120000.

use crate::errors::GameErrorCode;
use anchor_lang::prelude::*;

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
}
