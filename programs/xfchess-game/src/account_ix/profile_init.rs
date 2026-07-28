//! Profile initialization helpers that preserve existing gameplay state.

use crate::errors::GameErrorCode;
use crate::state::PlayerProfile;
use anchor_lang::prelude::*;
use anchor_lang::{AccountDeserialize, Discriminator};

/// Deserializes an existing `PlayerProfile` from raw account data, or builds
/// a fresh default one if the discriminator doesn't match (uninitialized
/// account). Either way, preserves all gameplay/verification/external-link
/// fields already present — only `authority`, `created_at`, and a zero
/// `elo_rating` seed are touched here. See ADR 0005 (profile init is not
/// profile reset).
pub fn load_or_new_profile(data: &[u8], player: Pubkey, now: i64) -> Result<PlayerProfile> {
    let mut profile = if data.len() >= 8 && &data[..8] == PlayerProfile::DISCRIMINATOR {
        let mut reader = data;
        PlayerProfile::try_deserialize(&mut reader)?
    } else {
        PlayerProfile::default()
    };

    require!(
        profile.authority == Pubkey::default() || profile.authority == player,
        GameErrorCode::UnauthorizedAccess
    );

    if profile.authority == Pubkey::default() {
        profile.authority = player;
    }
    if profile.created_at == 0 {
        profile.created_at = now;
    }
    if profile.elo_rating == 0.0 {
        profile.elo_rating = crate::elo::rating::INITIAL_ELO_CENTISCALE as f64;
    }

    Ok(profile)
}

/// Overwrites only the identity fields (username, country, date of birth) on
/// a profile, deliberately kept separate from gameplay-stat mutation.
pub fn update_identity_fields(
    profile: &mut PlayerProfile,
    username: String,
    country: String,
    date_of_birth: i64,
) {
    profile.username = username;
    profile.username_set = true;
    profile.country = country;
    profile.date_of_birth = date_of_birth;
}
