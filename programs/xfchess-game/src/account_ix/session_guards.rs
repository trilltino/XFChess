//! Shared session spending helpers.

use crate::errors::GameErrorCode;
use anchor_lang::prelude::*;

pub fn checked_session_total(current: u64, spend: u64) -> Result<u64> {
    current
        .checked_add(spend)
        .ok_or_else(|| GameErrorCode::ArithmeticOverflow.into())
}

pub fn require_within_limit(total: u64, limit: u64, error: GameErrorCode) -> Result<()> {
    if total <= limit {
        Ok(())
    } else {
        Err(error.into())
    }
}
