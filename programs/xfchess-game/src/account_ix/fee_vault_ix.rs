//! Instructions for platform fee vault management and ELO updates.

use anchor_lang::prelude::*;
use crate::state::{PlatformFeeVault, PlayerSession};
use crate::errors::GameErrorCode;

// ─── Fee Vault Instructions ───────────────────────────────────────────────────

#[derive(Accounts)]
pub struct InitializeFeeVault<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        init,
        payer = authority,
        space = 8 + PlatformFeeVault::INIT_SPACE,
        seeds = [PlatformFeeVault::SEED],
        bump,
    )]
    pub fee_vault: Account<'info, PlatformFeeVault>,
    pub system_program: Program<'info, System>,
}

pub fn handler_initialize_fee_vault(
    ctx: Context<InitializeFeeVault>,
    host_wallet: Pubkey,
) -> Result<()> {
    let vault = &mut ctx.accounts.fee_vault;
    vault.host_wallet = host_wallet;
    vault.total_accumulated = 0;
    vault.auto_claim_threshold = PlatformFeeVault::DEFAULT_THRESHOLD;
    vault.claim_interval_seconds = PlatformFeeVault::DEFAULT_INTERVAL;
    vault.last_claim_at = Clock::get()?.unix_timestamp;
    vault.total_claimed = 0;
    vault.bump = ctx.bumps.fee_vault;
    Ok(())
}

#[derive(Accounts)]
pub struct CollectFee<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(
        mut,
        seeds = [PlatformFeeVault::SEED],
        bump = fee_vault.bump,
    )]
    pub fee_vault: Account<'info, PlatformFeeVault>,
    pub system_program: Program<'info, System>,
}

pub fn handler_collect_fee(ctx: Context<CollectFee>, amount: u64) -> Result<()> {
    // Transfer fee from payer to vault PDA
    anchor_lang::system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            anchor_lang::system_program::Transfer {
                from: ctx.accounts.payer.to_account_info(),
                to: ctx.accounts.fee_vault.to_account_info(),
            },
        ),
        amount,
    )?;
    ctx.accounts.fee_vault.total_accumulated = ctx.accounts.fee_vault.total_accumulated
        .checked_add(amount)
        .ok_or(GameErrorCode::Overflow)?;
    Ok(())
}

#[derive(Accounts)]
pub struct ClaimFees<'info> {
    /// Anyone can trigger claim — permissionless
    #[account(mut)]
    pub caller: Signer<'info>,
    #[account(
        mut,
        seeds = [PlatformFeeVault::SEED],
        bump = fee_vault.bump,
    )]
    pub fee_vault: Account<'info, PlatformFeeVault>,
    /// CHECK: destination wallet verified against vault.host_wallet
    #[account(mut, constraint = host_wallet.key() == fee_vault.host_wallet)]
    pub host_wallet: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler_claim_fees(ctx: Context<ClaimFees>) -> Result<u64> {
    let now = Clock::get()?.unix_timestamp;
    let vault = &mut ctx.accounts.fee_vault;

    require!(vault.should_claim(now), GameErrorCode::NoPrizeToClaim);
    require!(vault.total_accumulated > 0, GameErrorCode::NoPrizeToClaim);

    let amount = vault.total_accumulated;

    // Transfer out of vault PDA
    **vault.to_account_info().try_borrow_mut_lamports()? -= amount;
    **ctx.accounts.host_wallet.try_borrow_mut_lamports()? += amount;

    vault.total_claimed = vault.total_claimed.checked_add(amount).ok_or(GameErrorCode::Overflow)?;
    vault.total_accumulated = 0;
    vault.last_claim_at = now;

    msg!("Claimed {} lamports to host wallet", amount);
    Ok(amount)
}

// ─── Player Session Instructions ──────────────────────────────────────────────

#[derive(Accounts)]
#[instruction(session_key: Pubkey)]
pub struct CreateSession<'info> {
    #[account(mut)]
    pub player: Signer<'info>,
    #[account(
        init,
        payer = player,
        space = 8 + PlayerSession::INIT_SPACE,
        seeds = [PlayerSession::SEED, player.key().as_ref(), session_key.as_ref()],
        bump,
    )]
    pub session: Account<'info, PlayerSession>,
    pub system_program: Program<'info, System>,
}

pub fn handler_create_session(
    ctx: Context<CreateSession>,
    session_key: Pubkey,
    duration: Option<i64>,
    spending_limit: Option<u64>,
    max_wager: Option<u64>,
) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let s = &mut ctx.accounts.session;
    s.player = ctx.accounts.player.key();
    s.session_key = session_key;
    s.expires_at = now + duration.unwrap_or(PlayerSession::DEFAULT_DURATION);
    s.spending_limit = spending_limit.unwrap_or(PlayerSession::DEFAULT_SPENDING_LIMIT);
    s.max_wager = max_wager.unwrap_or(PlayerSession::MAX_WAGER_DEFAULT);
    s.total_spent = 0;
    s.games_played = 0;
    s.can_create_games = true;
    s.can_join_games = true;
    s.can_claim_prizes = true;
    s.is_active = true;
    s.bump = ctx.bumps.session;
    Ok(())
}

#[derive(Accounts)]
pub struct RevokeSession<'info> {
    #[account(mut)]
    pub player: Signer<'info>,
    #[account(
        mut,
        close = player,
        has_one = player,
        seeds = [PlayerSession::SEED, player.key().as_ref(), session.session_key.as_ref()],
        bump = session.bump,
    )]
    pub session: Account<'info, PlayerSession>,
}

pub fn handler_revoke_session(ctx: Context<RevokeSession>) -> Result<()> {
    ctx.accounts.session.is_active = false;
    Ok(())
}

#[derive(Accounts)]
pub struct UpdateElo<'info> {
    /// Only the program authority (VPS) can update ELO
    pub authority: Signer<'info>,
    #[account(mut)]
    pub profile: Account<'info, crate::state::PlayerProfile>,
}

pub fn handler_update_elo(
    ctx: Context<UpdateElo>,
    _opponent_rating: u32,
    _opponent_rd: u32,
    outcome: u32, // 10000=win, 5000=draw, 0=loss
    is_ranked: bool,
    wager: u64,
    won_amount: u64,
) -> Result<()> {
    let p = &mut ctx.accounts.profile;
    // ELO updates are now handled in finalize_game with Glicko-2
    // This instruction is deprecated - kept for backward compatibility

    match outcome {
        10000 => {
            p.wins += 1;
            p.win_streak += 1;
            if p.win_streak > p.best_streak { p.best_streak = p.win_streak; }
        }
        0 => {
            p.losses += 1;
            p.win_streak = 0;
        }
        _ => {
            p.draws += 1;
            p.win_streak = 0;
        }
    }
    p.games_played += 1;
    p.last_game_at = Clock::get()?.unix_timestamp;

    if is_ranked {
        p.ranked_games += 1;
        p.total_wagered = p.total_wagered.saturating_add(wager);
        p.total_won = p.total_won.saturating_add(won_amount);
    }
    Ok(())
}
