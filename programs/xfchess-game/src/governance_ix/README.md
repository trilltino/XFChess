# Governance Instructions

Solana program instructions for governance operations, including administrative functions, configuration updates, and fee management.

## Overview

The governance instructions module provides administrative capabilities for tournament management. These instructions are restricted to tournament authorities and enable configuration changes, emergency actions, and fee adjustments.

## Governance Model

XFChess uses a centralized governance model where:
- **Tournament Authority** - The creator of a tournament has full administrative control
- **Authority Validation** - All governance instructions validate the signer is the tournament authority
- **State Restrictions** - Some actions can only be performed in specific tournament states
- **Safety Checks** - Configuration changes have validation to prevent invalid states

## Components

- Administrative functions for tournament management
- Configuration updates for tournament parameters
- Fee management for entry fees and platform fees
- Emergency pause and cancellation capabilities

## Example: Updating Configuration

This instruction allows the tournament authority to update tournament configuration.

```rust
use anchor_lang::prelude::*;

/// Context for updating tournament configuration
#[derive(Accounts)]
pub struct UpdateConfig<'info> {
    #[account(
        mut,
        has_one = authority
    )]
    pub tournament: Account<'info, Tournament>,
    
    pub authority: Signer<'info>,
}

/// Updates tournament configuration
/// 
/// This instruction allows the authority to update tournament parameters
/// such as entry fee and prize pool. Changes can only be made when the
/// tournament is not in progress.
/// 
/// # Arguments
/// * `ctx` - The instruction context
/// * `new_entry_fee` - New entry fee (optional)
/// * `new_prize_pool` - New prize pool (optional)
/// 
/// # Errors
/// - `Unauthorized` - Signer is not the tournament authority
/// - `TournamentInProgress` - Tournament is currently in progress
pub fn update_config(
    ctx: Context<UpdateConfig>,
    new_entry_fee: Option<u64>,
    new_prize_pool: Option<u64>,
) -> Result<()> {
    let tournament = &mut ctx.accounts.tournament;
    
    // Only authority can update config
    require!(
        ctx.accounts.authority.key() == tournament.authority,
        ErrorCode::Unauthorized
    );
    
    // Only allow updates when tournament is not in progress
    require!(
        tournament.status != TournamentStatus::InProgress,
        ErrorCode::TournamentInProgress
    );
    
    // Update configuration
    if let Some(fee) = new_entry_fee {
        tournament.entry_fee = fee;
    }
    if let Some(pool) = new_prize_pool {
        tournament.prize_pool = pool;
    }
    
    msg!("Tournament configuration updated");
    Ok(())
}
```

## Example: Emergency Pause

This instruction allows the authority to pause a tournament in case of emergencies.

```rust
/// Context for emergency pause
#[derive(Accounts)]
pub struct EmergencyPause<'info> {
    #[account(
        mut,
        has_one = authority
    )]
    pub tournament: Account<'info, Tournament>,
    
    pub authority: Signer<'info>,
    
    pub clock: Sysvar<'info, Clock>,
}

/// Pauses the tournament in case of emergency
/// 
/// This instruction pauses the tournament, preventing any further
/// match play until it is resumed. This is useful for handling
/// emergencies, bugs, or other unexpected situations.
/// 
/// # Arguments
/// * `ctx` - The instruction context
/// 
/// # Errors
/// - `Unauthorized` - Signer is not the tournament authority
/// - `AlreadyPaused` - Tournament is already paused
pub fn emergency_pause(ctx: Context<EmergencyPause>) -> Result<()> {
    let tournament = &mut ctx.accounts.tournament;
    
    // Only authority can pause
    require!(
        ctx.accounts.authority.key() == tournament.authority,
        ErrorCode::Unauthorized
    );
    
    // Check not already paused
    require!(
        tournament.status != TournamentStatus::Paused,
        ErrorCode::AlreadyPaused
    );
    
    // Pause tournament
    tournament.status = TournamentStatus::Paused;
    tournament.paused_at = Clock::get()?.unix_timestamp;
    
    msg!("Tournament paused by authority");
    Ok(())
}
```

## Example: Updating Fee Structure

This instruction allows the authority to update platform fee parameters.

```rust
/// Context for updating fee structure
#[derive(Accounts)]
pub struct UpdateFees<'info> {
    #[account(
        mut,
        has_one = authority
    )]
    pub config: Account<'info, Config>,
    
    pub authority: Signer<'info>,
}

/// Configuration account for fee parameters
#[account]
pub struct Config {
    /// Authority who can update configuration
    pub authority: Pubkey,
    
    /// Platform fee percentage (0-10%)
    pub platform_fee_percent: u8,
    
    /// Minimum entry fee in lamports
    pub min_entry_fee: u64,
    
    /// Maximum entry fee in lamports
    pub max_entry_fee: u64,
    
    /// Bump seed for PDA derivation
    pub bump: u8,
}

/// Updates fee structure
/// 
/// This instruction allows the authority to update platform fee parameters.
/// Fee changes are validated to ensure they remain within reasonable ranges.
/// 
/// # Arguments
/// * `ctx` - The instruction context
/// * `new_platform_fee` - New platform fee percentage (optional)
/// * `new_min_fee` - New minimum entry fee (optional)
/// * `new_max_fee` - New maximum entry fee (optional)
/// 
/// # Errors
/// - `Unauthorized` - Signer is not the config authority
/// - `FeeTooHigh` - Platform fee exceeds maximum (10%)
/// - `InvalidFeeRange` - Min fee is not less than max fee
pub fn update_fees(
    ctx: Context<UpdateFees>,
    new_platform_fee: Option<u8>,
    new_min_fee: Option<u64>,
    new_max_fee: Option<u64>,
) -> Result<()> {
    let config = &mut ctx.accounts.config;
    
    // Only authority can update fees
    require!(
        ctx.accounts.authority.key() == config.authority,
        ErrorCode::Unauthorized
    );
    
    // Validate fee ranges
    if let Some(fee) = new_platform_fee {
        require!(fee <= 10, ErrorCode::FeeTooHigh); // Max 10%
        config.platform_fee_percent = fee;
    }
    if let Some(min) = new_min_fee {
        require!(min < config.max_entry_fee, ErrorCode::InvalidFeeRange);
        config.min_entry_fee = min;
    }
    if let Some(max) = new_max_fee {
        require!(max > config.min_entry_fee, ErrorCode::InvalidFeeRange);
        config.max_entry_fee = max;
    }
    
    msg!("Fee structure updated");
    Ok(())
}
```

## Example: Tournament Cancellation

This instruction allows the authority to cancel a tournament and refund entry fees.

```rust
/// Context for canceling a tournament
#[derive(Accounts)]
pub struct CancelTournament<'info> {
    #[account(
        mut,
        close = authority
    )]
    pub tournament: Account<'info, Tournament>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

/// Cancels a tournament and refunds entry fees
/// 
/// This instruction:
/// 1. Validates the tournament can be cancelled
/// 2. Refunds entry fees to all registered players
/// 3. Closes the tournament account
/// 
/// # Arguments
/// * `ctx` - The instruction context
/// 
/// # Errors
/// - `Unauthorized` - Signer is not the tournament authority
/// - `CannotCancel` - Tournament cannot be cancelled (already in progress)
pub fn cancel_tournament(ctx: Context<CancelTournament>) -> Result<()> {
    let tournament = &ctx.accounts.tournament;
    
    // Only authority can cancel
    require!(
        ctx.accounts.authority.key() == tournament.authority,
        ErrorCode::Unauthorized
    );
    
    // Only allow cancellation before tournament starts
    require!(
        tournament.status != TournamentStatus::InProgress,
        ErrorCode::CannotCancel
    );
    
    // In a real implementation, this would refund entry fees to all players
    // For simplicity, we just close the account here
    
    msg!("Tournament cancelled");
    Ok(())
}
