//! Streaming prize claim with vesting (linear or cliff)

use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount};
use crate::state::{Tournament, TournamentStatus, PayoutType, VestingParams};
use crate::errors::GameErrorCode;

#[derive(Accounts)]
pub struct ClaimStreamingPrize<'info> {
    #[account(
        mut,
        seeds = [b"tournament", &tournament.tournament_id.to_le_bytes()],
        bump = tournament.bump,
        constraint = tournament.status == TournamentStatus::Completed @ GameErrorCode::TournamentNotCompleted,
    )]
    pub tournament: Account<'info, Tournament>,
    
    /// Prize vault for SPL tokens (or system account for SOL)
    #[account(mut)]
    pub prize_vault: Account<'info, TokenAccount>,
    
    /// Winner's token account
    #[account(mut)]
    pub winner_token_account: Account<'info, TokenAccount>,
    
    /// CHECK: Winner must be in tournament winners list
    #[account(
        constraint = 
            tournament.winner == Some(winner.key()) ||
            tournament.second_place == Some(winner.key()) ||
            tournament.third_place == Some(winner.key()) ||
            tournament.fourth_place == Some(winner.key())
        @ GameErrorCode::NotTournamentWinner
    )]
    pub winner: Signer<'info>,
    
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub clock: Sysvar<'info, Clock>,
}

pub fn handler(ctx: Context<ClaimStreamingPrize>) -> Result<()> {
    let tournament = &ctx.accounts.tournament;
    let clock = &ctx.accounts.clock;
    
    // Verify vesting is configured
    let vesting = tournament.vesting_params.as_ref()
        .ok_or(GameErrorCode::NoVestingConfigured)?;
    
    let now = clock.unix_timestamp;
    let elapsed = now.saturating_sub(vesting.start_time);
    
    // Calculate vested amount
    let total_prize = tournament.prize_pool;
    let claimable = calculate_vested_amount(
        total_prize,
        elapsed,
        vesting.duration_seconds,
        vesting.cliff_seconds,
        tournament.payout_type,
    )?;
    
    require!(claimable > 0, GameErrorCode::NoPrizeToClaim);
    
    // Transfer claimable amount
    let cpi_accounts = token::Transfer {
        from: ctx.accounts.prize_vault.to_account_info(),
        to: ctx.accounts.winner_token_account.to_account_info(),
        authority: ctx.accounts.tournament.to_account_info(),
    };
    
    let bump = tournament.bump;
    let seeds = &[
        b"tournament",
        &tournament.tournament_id.to_le_bytes()[..],
        &[bump],
    ];
    let signer = &[&seeds[..]];
    
    let cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        cpi_accounts,
        signer,
    );
    
    token::transfer(cpi_ctx, claimable)?;
    
    // Update claimed amount (would need to track in tournament state)
    // For now, emit event for tracking
    
    msg!("Claimed {} of {} prize ({}% vested)", 
        claimable, 
        total_prize,
        (claimable as f64 / total_prize as f64 * 100.0) as u64
    );
    
    Ok(())
}

/// Calculate vested amount based on payout type
fn calculate_vested_amount(
    total: u64,
    elapsed: i64,
    duration: i64,
    cliff: Option<i64>,
    payout_type: PayoutType,
) -> Result<u64> {
    if elapsed <= 0 {
        return Ok(0);
    }
    
    match payout_type {
        PayoutType::LumpSum => Ok(total),
        PayoutType::StreamingLinear => {
            if elapsed >= duration {
                Ok(total)
            } else {
                // Linear vesting: elapsed / duration * total
                Ok((total as u128)
                    .checked_mul(elapsed as u128)
                    .ok_or(GameErrorCode::MathOverflow)?
                    .checked_div(duration as u128)
                    .ok_or(GameErrorCode::MathOverflow)? as u64)
            }
        }
        PayoutType::StreamingCliff => {
            let cliff_seconds = cliff.ok_or(GameErrorCode::NoVestingConfigured)?;
            
            if elapsed < cliff_seconds {
                // Before cliff - nothing vested
                Ok(0)
            } else if elapsed >= duration {
                // After full duration - everything vested
                Ok(total)
            } else {
                // After cliff but before full duration
                // Linear vesting from cliff point
                let post_cliff_elapsed = elapsed - cliff_seconds;
                let post_cliff_duration = duration - cliff_seconds;
                
                Ok((total as u128)
                    .checked_mul(post_cliff_elapsed as u128)
                    .ok_or(GameErrorCode::MathOverflow)?
                    .checked_div(post_cliff_duration as u128)
                    .ok_or(GameErrorCode::MathOverflow)? as u64)
            }
        }
    }
}
