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
