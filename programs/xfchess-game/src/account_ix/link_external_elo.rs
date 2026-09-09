use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;

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

    pub player: AccountInfo<'info>,

    #[account(
        init_if_needed,
        payer = link_authority,
        space = LichessUsernameRecord::LEN,
        seeds = [LICHESS_USERNAME_SEED, username.as_bytes()],
        bump
    )]
    pub lichess_username_record: Account<'info, LichessUsernameRecord>,

    #[account(
        mut,
        signer,
        address = crate::constants::link_authority::ID @ GameErrorCode::UnauthorizedAccess
    )]
    pub link_authority: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

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

    // Deliberately does not touch any elo_* field. Linking is identity
    // verification plus a reference snapshot of your Lichess ratings — it no
    // longer seeds or otherwise influences any XFChess rating, which is
    // earned exclusively from XFChess games (see `lifecycle::settlement`).

    Ok(())
}
