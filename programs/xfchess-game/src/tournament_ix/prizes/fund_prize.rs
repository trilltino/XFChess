//! Instruction for operator to deposit USDC into the prize escrow.
//! Must be called before registration opens to lock the prize pool.

use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};

#[derive(Accounts)]
#[instruction(tournament_id: u64, amount: u64)]
pub struct FundUsdcPrize<'info> {
    #[account(
        mut,
        seeds = [TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        bump = tournament.bump,
        constraint = tournament.usdc_prize_mint.is_some() @ GameErrorCode::InvalidGameStatus,
    )]
    pub tournament: Account<'info, Tournament>,
    /// CHECK: USDC prize escrow PDA — the authority of the token account.
    #[account(
        seeds = [TOURNAMENT_USDC_PRIZE_SEED, &tournament_id.to_le_bytes()],
        bump
    )]
    pub usdc_prize_escrow_authority: UncheckedAccount<'info>,
    /// USDC prize escrow token account (holds the USDC prize pool).
    #[account(
        mut,
        associated_token::mint = usdc_mint,
        associated_token::authority = usdc_prize_escrow_authority,
    )]
    pub usdc_prize_escrow: Account<'info, TokenAccount>,
    /// Operator's USDC ATA — source of the funds.
    #[account(
        mut,
        constraint = operator_usdc_ata.owner == operator.key() @ GameErrorCode::UnauthorizedAccess,
        constraint = operator_usdc_ata.mint == usdc_mint.key() @ GameErrorCode::InvalidGameStatus,
    )]
    pub operator_usdc_ata: Account<'info, TokenAccount>,
    /// The USDC mint account.
    #[account(
        constraint = usdc_mint.key() == tournament.usdc_prize_mint.unwrap() @ GameErrorCode::InvalidMint
    )]
    pub usdc_mint: Account<'info, token::Mint>,
    /// Operator funding the prize pool.
    #[account(mut)]
    pub operator: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<FundUsdcPrize>, tournament_id: u64, amount: u64) -> Result<()> {
    require!(amount > 0, GameErrorCode::InvalidGameStatus);
    require!(
        !ctx.accounts.tournament.usdc_prize_funded,
        GameErrorCode::InvalidGameStatus
    );

    let tournament = &mut ctx.accounts.tournament;

    // Verify the tournament is still in registration phase
    require!(
        tournament.status == TournamentStatus::Registration,
        GameErrorCode::TournamentNotInRegistration
    );

    // Transfer USDC from operator to escrow
    let transfer_instruction = Transfer {
        from: ctx.accounts.operator_usdc_ata.to_account_info(),
        to: ctx.accounts.usdc_prize_escrow.to_account_info(),
        authority: ctx.accounts.operator.to_account_info(),
    };

    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            transfer_instruction,
        ),
        amount,
    )?;

    // Update tournament state
    tournament.usdc_prize_pool = amount;
    tournament.usdc_prize_funded = true;

    msg!(
        "Tournament {} USDC prize pool funded: {} USDC (6 decimals)",
        tournament_id,
        amount
    );

    Ok(())
}
