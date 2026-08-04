//! Shared prize-place and payout math helpers.

use crate::errors::GameErrorCode;
use crate::state::Tournament;
use anchor_lang::prelude::*;

/// Top-N places a tournament can pay a prize share to.
pub const MAX_PRIZE_PLACES: usize = 10;

/// The tournament's placed players, 1st through 10th, in order.
pub fn places(tournament: &Tournament) -> [Option<Pubkey>; MAX_PRIZE_PLACES] {
    [
        tournament.winner,
        tournament.second_place,
        tournament.third_place,
        tournament.fourth_place,
        tournament.fifth_place,
        tournament.sixth_place,
        tournament.seventh_place,
        tournament.eighth_place,
        tournament.ninth_place,
        tournament.tenth_place,
    ]
}

/// The bit flag for place `index` within `Tournament::prizes_claimed`.
pub fn place_bit(index: usize) -> Result<u16> {
    require!(index < MAX_PRIZE_PLACES, GameErrorCode::InvalidArgument);
    Ok(1u16 << index)
}

/// `pool * share_bps / 10_000`, computed in u128 to avoid overflow before
/// narrowing back to lamports.
pub fn prize_amount(pool: u64, share_bps: u16) -> Result<u64> {
    let value = (pool as u128)
        .checked_mul(share_bps as u128)
        .and_then(|value| value.checked_div(10_000))
        .ok_or(GameErrorCode::ArithmeticOverflow)?;
    require!(value <= u64::MAX as u128, GameErrorCode::ArithmeticOverflow);
    Ok(value as u64)
}

/// Finds `claimant`'s place, returning `(place_index, share_bps)`.
pub fn find_place(tournament: &Tournament, claimant: Pubkey) -> Option<(usize, u16)> {
    places(tournament)
        .iter()
        .enumerate()
        .find(|(_, place)| **place == Some(claimant))
        .map(|(index, _)| (index, tournament.prize_shares[index]))
}

/// True if place `index` has a placed player, a nonzero prize share, and
/// hasn't been claimed yet — used to gate both individual claims and
/// `close_tournament`'s all-claimed check.
pub fn funded_place_unclaimed(tournament: &Tournament, index: usize) -> Result<bool> {
    let place = places(tournament)[index];
    let bit = place_bit(index)?;
    Ok(place.is_some()
        && tournament.prize_shares[index] > 0
        && tournament.prizes_claimed & bit == 0)
}
