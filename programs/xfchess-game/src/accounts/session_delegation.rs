use anchor_lang::prelude::*;
use crate::state::{Game, SessionDelegation};

#[derive(Accounts)]
#[instruction(game_id: u64)]
pub struct AuthorizeSessionKey<'info> {
    #[account(
        mut,
        seeds = [b"game", game_id.to_le_bytes().as_ref()],
        bump = game.bump,
    )]
    pub game: Account<'info, Game>,
    
    #[account(
        init,
        payer = player,
        seeds = [b"session_delegation", game_id.to_le_bytes().as_ref(), player.key().as_ref()],
        bump,
        space = 8 + std::mem::size_of::<SessionDelegation>()
    )]
    pub session_delegation: Account<'info, SessionDelegation>,
    
    #[account(mut)]
    pub player: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(game_id: u64)]
pub struct RevokeSessionKey<'info> {
    #[account(
        mut,
        seeds = [b"game", game_id.to_le_bytes().as_ref()],
        bump = game.bump,
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

#[derive(Accounts)]
pub struct CommitMoveBatch<'info> {
    #[account(
        mut,
        seeds = [b"game", game.game_id.to_le_bytes().as_ref()],
        bump = game.bump,
    )]
    pub game: Account<'info, Game>,
    
    #[account(
        mut,
        seeds = [b"move_log", game.game_id.to_le_bytes().as_ref()],
        bump = move_log.bump,
    )]
    pub move_log: Account<'info, crate::state::move_log::MoveLog>,
    
    /// CHECK: Verified by constraint checking that the key matches the session_key in delegation
    #[account(mut)]
    pub white_session: Signer<'info>,
    
    /// CHECK: Verified by constraint checking that the key matches the session_key in delegation
    #[account(mut)]
    pub black_session: Signer<'info>,
    
    #[account(
        mut,
        seeds = [b"session_delegation", game.game_id.to_le_bytes().as_ref(), game.white.as_ref()],
        bump = white_delegation.bump,
    )]
    pub white_delegation: Account<'info, SessionDelegation>,
    
    #[account(
        mut,
        seeds = [b"session_delegation", game.game_id.to_le_bytes().as_ref(), game.black.as_ref()],
        bump = black_delegation.bump,
    )]
    pub black_delegation: Account<'info, SessionDelegation>,
    
    pub system_program: Program<'info, System>,
}