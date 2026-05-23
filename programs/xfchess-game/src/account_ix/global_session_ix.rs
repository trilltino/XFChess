//! Instructions: `authorize_global_session` and `revoke_global_session`.
//!
//! `authorize_global_session` creates (or re-creates) a
//! [`GlobalSessionDelegation`] PDA for `player`. After this call the session
//! key may co-sign `global_create_game` and `global_join_game` without a
//! wallet popup — for up to `DEFAULT_GAMES` games, within `spending_limit`
//! lamports, and until `expires_at`.
//!
//! `revoke_global_session` disables the key immediately (sets `enabled =
//! false` and `expires_at` to now). The PDA stays on-chain so the player can
//! call `authorize_global_session` again to refresh it.

use crate::errors::XfchessGameError;
use crate::state::GlobalSessionDelegation;
use anchor_lang::prelude::*;

/// Arguments for `authorize_global_session`.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug)]
pub struct AuthorizeGlobalSessionArgs {
    /// Hot key allowed to co-sign game instructions.
    pub session_key: Pubkey,
    /// Session lifetime in seconds. `None` → [`GlobalSessionDelegation::DEFAULT_DURATION`].
    pub duration_secs: Option<i64>,
    /// Total spending cap in lamports. `None` → [`GlobalSessionDelegation::DEFAULT_SPENDING_LIMIT`].
    pub spending_limit: Option<u64>,
    /// Per-game wager cap in lamports. `None` → [`GlobalSessionDelegation::DEFAULT_MAX_WAGER`].
    pub max_wager: Option<u64>,
    /// Number of games this session covers. `None` → [`GlobalSessionDelegation::DEFAULT_GAMES`].
    pub games: Option<u16>,
    /// SOL deposited into the delegation vault for gasless game funding.
    pub deposit_lamports: u64,
}

pub fn handler_authorize_global_session(
    ctx: Context<AuthorizeGlobalSessionCtx>,
    args: AuthorizeGlobalSessionArgs,
) -> Result<()> {
    let delegation = &mut ctx.accounts.session_delegation;
    let player = &ctx.accounts.player;

    // Reject if a valid session is already live (player must revoke first).
    let now = Clock::get()?.unix_timestamp;
    if delegation.enabled && now < delegation.expires_at && delegation.games_remaining > 0 {
        // Allow re-auth if it's a brand new account (player field is default).
        require!(
            delegation.player == Pubkey::default(),
            XfchessGameError::GlobalSessionAlreadyActive
        );
    }

    // Deposit lamports into the delegation vault (covers future wager transfers).
    if args.deposit_lamports > 0 {
        anchor_lang::system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: player.to_account_info(),
                    to: delegation.to_account_info(),
                },
            ),
            args.deposit_lamports,
        )?;
    }

    let duration = args
        .duration_secs
        .unwrap_or(GlobalSessionDelegation::DEFAULT_DURATION);
    require!(duration > 0, XfchessGameError::UnauthorizedAccess);
    let expires_at = now
        .checked_add(duration)
        .ok_or(XfchessGameError::MathOverflow)?;

    delegation.player = player.key();
    delegation.session_key = args.session_key;
    delegation.expires_at = expires_at;
    delegation.spending_limit = args
        .spending_limit
        .unwrap_or(GlobalSessionDelegation::DEFAULT_SPENDING_LIMIT);
    delegation.total_spent = 0;
    delegation.max_wager = args
        .max_wager
        .unwrap_or(GlobalSessionDelegation::DEFAULT_MAX_WAGER);
    delegation.games_remaining = args.games.unwrap_or(GlobalSessionDelegation::DEFAULT_GAMES);
    delegation.enabled = true;
    delegation.bump = ctx.bumps.session_delegation;

    Ok(())
}

pub fn handler_revoke_global_session(ctx: Context<RevokeGlobalSessionCtx>) -> Result<()> {
    let delegation = &mut ctx.accounts.session_delegation;
    let player = &ctx.accounts.player;

    require_keys_eq!(
        delegation.player,
        player.key(),
        XfchessGameError::UnauthorizedAccess
    );

    delegation.enabled = false;
    delegation.expires_at = Clock::get()?.unix_timestamp;
    Ok(())
}

#[derive(Accounts)]
pub struct AuthorizeGlobalSessionCtx<'info> {
    #[account(
        init_if_needed,
        payer = player,
        seeds = [GlobalSessionDelegation::SEED, player.key().as_ref()],
        bump,
        space = 8 + GlobalSessionDelegation::INIT_SPACE,
    )]
    pub session_delegation: Account<'info, GlobalSessionDelegation>,

    #[account(mut)]
    pub player: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RevokeGlobalSessionCtx<'info> {
    #[account(
        mut,
        seeds = [GlobalSessionDelegation::SEED, player.key().as_ref()],
        bump = session_delegation.bump,
    )]
    pub session_delegation: Account<'info, GlobalSessionDelegation>,

    #[account(mut)]
    pub player: Signer<'info>,
}
