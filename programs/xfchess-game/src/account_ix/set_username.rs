//! Instruction for setting or updating a username associated with a profile.

use crate::constants::*;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(username: String)]
pub struct SetUsername<'info> {
    #[account(
        mut,
        seeds = [PROFILE_SEED, player.key().as_ref()],
        bump,
        has_one = authority
    )]
    pub player_profile: Account<'info, PlayerProfile>,

    /// UsernameRecord PDA ensures uniqueness
    /// Seeds: [USERNAME_SEED, username.as_bytes()]
    #[account(
        init,
        payer = player,
        space = 8 + UsernameRecord::LEN,
        seeds = [USERNAME_SEED, username.as_bytes()],
        bump
    )]
    pub username_record: Account<'info, UsernameRecord>,

    #[account(mut)]
    pub player: Signer<'info>,

    /// CHECK: Player's authority (must match profile.authority)
    pub authority: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<SetUsername>, username: String) -> Result<()> {
    // Validate username format
    validate_username(&username)?;

    let profile = &mut ctx.accounts.player_profile;
    let record = &mut ctx.accounts.username_record;

    // Initialize username record
    record.owner = ctx.accounts.player.key();
    record.created_at = Clock::get()?.unix_timestamp;

    // Set username on profile
    profile.username = username;
    profile.username_set = true;

    Ok(())
}
