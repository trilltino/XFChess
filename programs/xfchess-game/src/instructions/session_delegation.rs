use crate::errors::XfchessGameError;
use crate::state::game::{Game, GameStatus, SessionDelegation};
use anchor_lang::prelude::*;

pub fn handler_authorize_session_key(
    ctx: Context<AuthorizeSessionCtx>,
    game_id: u64,
    session_pubkey: Pubkey,
) -> Result<()> {
    let game = &mut ctx.accounts.game;
    let session_delegation = &mut ctx.accounts.session_delegation;
    let player = &ctx.accounts.player;

    // Verify that the caller is either the white or black player in the game
    require!(
        player.key() == game.white || player.key() == game.black,
        XfchessGameError::UnauthorizedAccess
    );

    // Set up the session delegation
    session_delegation.game_id = game_id;
    session_delegation.player = player.key();
    session_delegation.session_key = session_pubkey;
    session_delegation.expires_at = Clock::get()?.unix_timestamp + (2 * 60 * 60); // 2 hours from now
    session_delegation.max_batch_len = 10;
    session_delegation.enabled = true;
    session_delegation.bump = ctx.bumps.session_delegation;

    Ok(())
}

pub fn handler_revoke_session_key(ctx: Context<RevokeSessionCtx>, game_id: u64) -> Result<()> {
    let session_delegation = &mut ctx.accounts.session_delegation;
    let player = &ctx.accounts.player;

    // Verify that the caller is the owner of the delegation
    require!(
        player.key() == session_delegation.player,
        XfchessGameError::UnauthorizedAccess
    );

    // Disable the session delegation
    session_delegation.enabled = false;
    session_delegation.expires_at = Clock::get()?.unix_timestamp; // Set to now to expire immediately

    Ok(())
}

#[derive(Accounts)]
#[instruction(game_id: u64)]
pub struct AuthorizeSessionCtx<'info> {
    #[account(
        mut,
        seeds = [b"game", game_id.to_le_bytes().as_ref()],
        bump,
    )]
    pub game: Account<'info, Game>,

    #[account(
        init,
        payer = player,
        seeds = [b"session_delegation", game_id.to_le_bytes().as_ref(), player.key().as_ref()],
        bump,
        space = 8 + std::mem::size_of::<SessionDelegation>() + 8
    )]
    pub session_delegation: Account<'info, SessionDelegation>,

    #[account(mut)]
    pub player: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(game_id: u64)]
pub struct RevokeSessionCtx<'info> {
    #[account(
        mut,
        seeds = [b"game", game_id.to_le_bytes().as_ref()],
        bump,
    )]
    pub game: Account<'info, Game>,

    #[account(
        mut,
        seeds = [b"session_delegation", game_id.to_le_bytes().as_ref(), player.key().as_ref()],
        bump = session_delegation.bump,
    )]
    pub session_delegation: Account<'info, SessionDelegation>,

    #[account(mut)]
    pub player: Signer<'info>,

    pub system_program: Program<'info, System>,
}
