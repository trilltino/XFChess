//! Instruction for linking a verified Lichess account to a player profile.
//!
//! The backend verifies ownership via bio-nonce, then signs this instruction
//! with the `link_authority` keypair to write the attestation on-chain.

use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;

/// Writes a verified Lichess rating attestation onto a player's profile.
/// Only the configured `link_authority` (VPS backend signer) may call this —
/// players cannot self-report ratings.
#[derive(Accounts)]
#[instruction(
    username: String,
    blitz_rating: u32,
    rapid_rating: u32,
    bullet_rating: u32,
)]
pub struct LinkExternalElo<'info> {
    #[account(
        mut,
        seeds = [PROFILE_SEED, player.key().as_ref()],
        bump,
    )]
    pub player_profile: Account<'info, PlayerProfile>,

    /// CHECK: We just need their pubkey to form the seed and verify authority.
    pub player: AccountInfo<'info>,

    /// Claims (or verifies existing ownership of) this Lichess username's
    /// uniqueness lock — see `LichessUsernameRecord`'s own doc comment for
    /// why this exists. `init_if_needed` because the first link for a given
    /// Lichess username creates the record; a re-sync of an already-linked
    /// account reuses the same one.
    #[account(
        init_if_needed,
        payer = link_authority,
        space = LichessUsernameRecord::LEN,
        seeds = [LICHESS_USERNAME_SEED, username.as_bytes()],
        bump
    )]
    pub lichess_username_record: Account<'info, LichessUsernameRecord>,

    /// CHECK: The external-elo linking authority (VPS backend signer).
    /// `mut` because it pays for `lichess_username_record` on first link.
    #[account(
        mut,
        signer,
        address = crate::constants::link_authority::ID @ GameErrorCode::UnauthorizedAccess
    )]
    pub link_authority: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

/// Records the Lichess username and blitz/rapid/bullet ratings (converted to
/// centiscale) on the caller's profile. On the first link only, also seeds
/// `elo_rating` from the external rating (rapid, unless blitz is 500+ higher).
pub fn handler(
    ctx: Context<LinkExternalElo>,
    username: String,
    blitz_rating: u32,
    rapid_rating: u32,
    bullet_rating: u32,
) -> Result<()> {
    // Guard against an unconfigured authority: the placeholder key is all-zeros
    // and must be replaced with the real backend signer before this instruction
    // is usable (no one can sign as the default pubkey, but fail loudly anyway).
    require!(
        crate::constants::link_authority::ID != Pubkey::default(),
        GameErrorCode::UnauthorizedAccess
    );

    let profile = &mut ctx.accounts.player_profile;

    // Ensure the profile belongs to the player
    require!(
        profile.authority == ctx.accounts.player.key(),
        GameErrorCode::UnauthorizedAccess
    );

    // Validate username length (Lichess usernames are 2-30 chars)
    require!(
        !username.is_empty() && username.len() <= 30,
        GameErrorCode::InvalidUsername
    );

    // Claim (or verify ownership of) the Lichess-username uniqueness
    // record — same first-claimant-wins model `account_ix::profile::handler`
    // already uses for local usernames via `UsernameRecord`. Must happen
    // before writing `profile.lichess_username` below so a rejected claim
    // (someone else already linked this exact Lichess handle) never leaves
    // the profile in a half-updated state.
    let record = &mut ctx.accounts.lichess_username_record;
    if record.owner == Pubkey::default() {
        record.owner = ctx.accounts.player.key();
        record.created_at = Clock::get()?.unix_timestamp;
    } else {
        require!(
            record.owner == ctx.accounts.player.key(),
            GameErrorCode::UsernameTaken
        );
    }

    let blitz_centiscale = crate::elo::rating::external_to_centiscale(blitz_rating)?;
    let rapid_centiscale = crate::elo::rating::external_to_centiscale(rapid_rating)?;
    let bullet_centiscale = crate::elo::rating::external_to_centiscale(bullet_rating)?;

    // Store Lichess linkage data
    profile.lichess_username = username;
    profile.lichess_verified = true;
    profile.lichess_blitz = blitz_centiscale;
    profile.lichess_rapid = rapid_centiscale;
    profile.lichess_bullet = bullet_centiscale;
    profile.lichess_last_sync = Clock::get()?.unix_timestamp;
    profile.external_elo_source = 1; // 1 = Lichess

    // Seed on-chain elo_rating if this is the first external link
    if !profile.seeded_from_external {
        // Use rapid as the default seed unless blitz is significantly higher
        let seed_rating = if blitz_rating > rapid_rating + 500 {
            blitz_rating
        } else {
            rapid_rating
        };
        profile.elo_rating = crate::elo::rating::external_to_centiscale(seed_rating)? as f64;
        profile.seeded_from_external = true;
    }

    Ok(())
}
