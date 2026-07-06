//! Shared prize-place and payout math helpers.

use crate::errors::GameErrorCode;
use crate::state::Tournament;
use anchor_lang::prelude::*;

pub const MAX_PRIZE_PLACES: usize = 10;

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

pub fn place_bit(index: usize) -> Result<u16> {
    require!(index < MAX_PRIZE_PLACES, GameErrorCode::InvalidArgument);
    Ok(1u16 << index)
}

pub fn prize_amount(pool: u64, share_bps: u16) -> Result<u64> {
    let value = (pool as u128)
        .checked_mul(share_bps as u128)
        .and_then(|value| value.checked_div(10_000))
        .ok_or(GameErrorCode::ArithmeticOverflow)?;
    require!(value <= u64::MAX as u128, GameErrorCode::ArithmeticOverflow);
    Ok(value as u64)
}

pub fn find_place(tournament: &Tournament, claimant: Pubkey) -> Option<(usize, u16)> {
    places(tournament)
        .iter()
        .enumerate()
        .find(|(_, place)| **place == Some(claimant))
        .map(|(index, _)| (index, tournament.prize_shares[index]))
}

pub fn funded_place_unclaimed(tournament: &Tournament, index: usize) -> Result<bool> {
    let place = places(tournament)[index];
    let bit = place_bit(index)?;
    Ok(place.is_some()
        && tournament.prize_shares[index] > 0
        && tournament.prizes_claimed & bit == 0)
}
