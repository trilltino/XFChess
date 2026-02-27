use crate::constants::*;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct InitProfile<'info> {
    #[account(
        init, 
        payer = player, 
        space = 8 + PlayerProfile::INIT_SPACE, 
        seeds = [PROFILE_SEED, player.key().as_ref()], 
        bump
    )]
    pub player_profile: Account<'info, PlayerProfile>,
    #[account(mut)]
    pub player: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<InitProfile>) -> Result<()> {
    let profile = &mut ctx.accounts.player_profile;
    profile.authority = ctx.accounts.player.key();
    profile.elo = 1200;
    profile.games_played = 0;
    profile.wins = 0;
    profile.losses = 0;
    profile.draws = 0;
    Ok(())
}
