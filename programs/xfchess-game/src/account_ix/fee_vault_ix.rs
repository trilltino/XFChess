//! Instructions for platform fee vault management and ELO updates.

use crate::errors::GameErrorCode;
use crate::state::{PlatformFeeVault, PlayerSession};
use anchor_lang::prelude::*;

// ─── Fee Vault Instructions ───────────────────────────────────────────────────

/// Creates the single, program-wide `PlatformFeeVault` PDA that accumulates
/// platform fees collected from wagered games.
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

/// Sets `host_wallet` as the fee vault's payout destination and applies the
/// default auto-claim threshold/interval.
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

/// Deposits a platform fee into the vault. Restricted to the VPS backend
/// authority — this is called by the backend when settling a wagered game,
/// not directly by players.
#[derive(Accounts)]
pub struct CollectFee<'info> {
    /// VPS backend authority — the only signer allowed to deposit into the fee vault.
    #[account(mut, address = crate::constants::vps_authority::ID @ GameErrorCode::UnauthorizedAccess)]
    pub payer: Signer<'info>,
    #[account(
        mut,
        seeds = [PlatformFeeVault::SEED],
        bump = fee_vault.bump,
    )]
    pub fee_vault: Account<'info, PlatformFeeVault>,
    pub system_program: Program<'info, System>,
}

/// Transfers `amount` from the VPS authority into the fee vault and adds it
/// to `total_accumulated`.
pub fn handler_collect_fee(ctx: Context<CollectFee>, amount: u64) -> Result<()> {
    // Transfer fee from payer to vault PDA
    anchor_lang::system_program::transfer(
        CpiContext::new(
            System::id(),
            anchor_lang::system_program::Transfer {
                from: ctx.accounts.payer.to_account_info(),
                to: ctx.accounts.fee_vault.to_account_info(),
            },
        ),
        amount,
    )?;
    ctx.accounts.fee_vault.total_accumulated = ctx
        .accounts
        .fee_vault
        .total_accumulated
        .checked_add(amount)
        .ok_or(GameErrorCode::Overflow)?;
    Ok(())
}

/// Sweeps accumulated fees to `host_wallet`. Permissionless — any signer may
/// trigger a claim, but funds only ever move to the vault's configured host
/// wallet, and only once `should_claim` (threshold/interval) is satisfied.
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

/// Pays out the vault's full accumulated balance to `host_wallet` and resets
/// `total_accumulated` to 0. Moves lamports directly (not via a system-program
/// CPI) since the vault is program-owned. Returns the amount claimed.
pub fn handler_claim_fees(ctx: Context<ClaimFees>) -> Result<u64> {
    let now = Clock::get()?.unix_timestamp;

    // Read state and validate before any mutable borrow.
    require!(
        ctx.accounts.fee_vault.should_claim(now),
        GameErrorCode::NoPrizeToClaim
    );
    require!(
        ctx.accounts.fee_vault.total_accumulated > 0,
        GameErrorCode::NoPrizeToClaim
    );

    let amount = ctx.accounts.fee_vault.total_accumulated;

    // Capture AccountInfo values before the mutable data borrow below.
    let vault_info = ctx.accounts.fee_vault.to_account_info();
    let rent_min = Rent::get()?.minimum_balance(vault_info.data_len());
    require!(
        vault_info.lamports().saturating_sub(amount) >= rent_min,
        GameErrorCode::Overflow
    );

    // Direct lamport manipulation is correct here — fee_vault is program-owned
    // so the system program cannot CPI-transfer on its behalf.
    **vault_info.try_borrow_mut_lamports()? -= amount;
    **ctx.accounts.host_wallet.try_borrow_mut_lamports()? += amount;

    // Re-borrow for data mutation after lamport manipulation is complete.
    let vault = &mut ctx.accounts.fee_vault;
    vault.total_claimed = vault
        .total_claimed
        .checked_add(amount)
        .ok_or(GameErrorCode::Overflow)?;
    vault.total_accumulated = 0;
    vault.last_claim_at = now;

    Ok(amount)
}

// ─── Player Session Instructions ──────────────────────────────────────────────

/// Creates a `PlayerSession` PDA: a time-bounded (24h default) session key
/// that can sign game transactions without a wallet popup. Distinct from the
/// longer-lived, multi-game session delegation in `global_session_ix.rs`.
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

/// Initializes the session with the given (or default) duration, spending
/// limit, and max-wager cap, and grants create/join/claim permissions.
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

/// Closes a `PlayerSession` PDA and refunds its rent to the player.
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

/// Marks the session inactive; the account then closes per the `close = player`
/// constraint on `RevokeSession`, returning rent to the player.
pub fn handler_revoke_session(ctx: Context<RevokeSession>) -> Result<()> {
    ctx.accounts.session.is_active = false;
    Ok(())
}

/// Updates a player's lifetime win/loss/draw/wager stats after a game.
/// Restricted to the VPS backend authority.
#[derive(Accounts)]
pub struct UpdateElo<'info> {
    /// VPS backend authority — only this key may update ELO standings.
    #[account(address = crate::constants::vps_authority::ID @ GameErrorCode::UnauthorizedAccess)]
    pub authority: Signer<'info>,
    #[account(mut)]
    pub profile: Account<'info, crate::state::PlayerProfile>,
}

/// Updates lifetime stats (wins/losses/draws/streaks/games_played) and, for
/// ranked games, wager totals. Does not touch `elo_rating` itself — that field
/// is updated exclusively by `finalize_game` (K=32). `_opponent_rating` and
/// `_opponent_rd` are unused here and kept only for ABI compatibility.
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
    // ELO rating (elo_rating field) is updated exclusively in finalize_game using K=32.
    // This instruction updates lifetime stats only (wins/losses/draws/wagered/won).
    // The _opponent_rating and _opponent_rd parameters are kept for ABI compatibility.

    match outcome {
        10000 => {
            p.wins = p
                .wins
                .checked_add(1)
                .ok_or(GameErrorCode::ArithmeticOverflow)?;
            p.win_streak = p
                .win_streak
                .checked_add(1)
                .ok_or(GameErrorCode::ArithmeticOverflow)?;
            if p.win_streak > p.best_streak {
                p.best_streak = p.win_streak;
            }
        }
        0 => {
            p.losses = p
                .losses
                .checked_add(1)
                .ok_or(GameErrorCode::ArithmeticOverflow)?;
            p.win_streak = 0;
        }
        _ => {
            p.draws = p
                .draws
                .checked_add(1)
                .ok_or(GameErrorCode::ArithmeticOverflow)?;
            p.win_streak = 0;
        }
    }
    p.games_played = p
        .games_played
        .checked_add(1)
        .ok_or(GameErrorCode::ArithmeticOverflow)?;
    p.last_game_at = Clock::get()?.unix_timestamp;

    if is_ranked {
        p.ranked_games = p
            .ranked_games
            .checked_add(1)
            .ok_or(GameErrorCode::ArithmeticOverflow)?;
        p.total_wagered = p
            .total_wagered
            .checked_add(wager)
            .ok_or(GameErrorCode::ArithmeticOverflow)?;
        p.total_won = p
            .total_won
            .checked_add(won_amount)
            .ok_or(GameErrorCode::ArithmeticOverflow)?;
    }
    Ok(())
}
