//! Shared session spending helpers.

use crate::errors::GameErrorCode;
use anchor_lang::prelude::*;

/// Adds `spend` to `current`, rejecting overflow instead of saturating —
/// a saturated value could silently let a session spend more than intended.
pub fn checked_session_total(current: u64, spend: u64) -> Result<u64> {
    current
        .checked_add(spend)
        .ok_or_else(|| GameErrorCode::ArithmeticOverflow.into())
}

/// Fails with `error` if `total` exceeds `limit`.
pub fn require_within_limit(total: u64, limit: u64, error: GameErrorCode) -> Result<()> {
    if total <= limit {
        Ok(())
    } else {
        Err(error.into())
    }
}
