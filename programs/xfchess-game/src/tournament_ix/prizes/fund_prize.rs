use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, TransferChecked};

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
    #[account(
        seeds = [TOURNAMENT_USDC_PRIZE_SEED, &tournament_id.to_le_bytes()],
        bump
    )]
    pub usdc_prize_escrow_authority: UncheckedAccount<'info>,
    #[account(
        mut,
        associated_token::mint = usdc_mint,
        associated_token::authority = usdc_prize_escrow_authority,
    )]
    pub usdc_prize_escrow: Account<'info, TokenAccount>,
    #[account(
        mut,
        constraint = operator_usdc_ata.owner == operator.key() @ GameErrorCode::UnauthorizedAccess,
        constraint = operator_usdc_ata.mint == usdc_mint.key() @ GameErrorCode::InvalidGameStatus,
    )]
    pub operator_usdc_ata: Account<'info, TokenAccount>,
    #[account(
        constraint = usdc_mint.key() == tournament.usdc_prize_mint.ok_or(GameErrorCode::InvalidMint)? @ GameErrorCode::InvalidMint
    )]
    pub usdc_mint: Account<'info, token::Mint>,
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
    require!(
        tournament.tournament_id == tournament_id,
        GameErrorCode::UnauthorizedAccess
    );

    require!(
        tournament.status == TournamentStatus::Registration,
        GameErrorCode::TournamentNotInRegistration
    );

    require!(
        tournament.num_registered_players == 0,
        GameErrorCode::PrizeAlreadyFunded
    );

    let transfer_instruction = TransferChecked {
        from: ctx.accounts.operator_usdc_ata.to_account_info(),
        mint: ctx.accounts.usdc_mint.to_account_info(),
        to: ctx.accounts.usdc_prize_escrow.to_account_info(),
        authority: ctx.accounts.operator.to_account_info(),
    };

    token::transfer_checked(
        CpiContext::new(Token::id(), transfer_instruction),
        amount,
        ctx.accounts.usdc_mint.decimals,
    )?;

    tournament.usdc_prize_pool = amount;
    tournament.usdc_prize_funded = true;

    Ok(())
}
