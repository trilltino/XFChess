//! Instruction allowing winners to claim their tournament prize shares.
//! Supports USDC prize pools (primary) and SOL fallback.

use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use crate::tournament_ix::prizes::ledger;
use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};

#[derive(Accounts)]
#[instruction(tournament_id: u64)]
pub struct ClaimTournamentPrize<'info> {
    #[account(
        mut,
        seeds = [TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        bump = tournament.bump
    )]
    pub tournament: Account<'info, Tournament>,
    /// CHECK: USDC prize escrow PDA — the authority of the token account.
    #[account(
        seeds = [TOURNAMENT_USDC_PRIZE_SEED, &tournament_id.to_le_bytes()],
        bump
    )]
    pub usdc_prize_escrow_authority: UncheckedAccount<'info>,
    /// USDC prize escrow token account (only used if usdc_prize_mint is Some).
    #[account(
        mut,
        associated_token::mint = usdc_mint,
        associated_token::authority = usdc_prize_escrow_authority,
    )]
    pub usdc_prize_escrow: Option<Account<'info, TokenAccount>>,
    /// Claimant's USDC ATA — receives USDC prize (only used if usdc_prize_mint is Some).
    #[account(mut)]
    pub claimant_usdc_ata: Option<Account<'info, TokenAccount>>,
    /// The USDC mint account (only used if usdc_prize_mint is Some).
    pub usdc_mint: Option<Account<'info, token::Mint>>,
    /// CHECK: SOL escrow PDA (legacy, only used for SOL-only tournaments).
    #[account(
        mut,
        seeds = [TOURNAMENT_ESCROW_SEED, &tournament_id.to_le_bytes()],
        bump
    )]
    pub escrow_pda: UncheckedAccount<'info>,
    /// CHECK: Claimant's wallet — must match a winning position.
    #[account(mut, constraint = claimant_wallet.key() == claimant.key() @ GameErrorCode::UnauthorizedAccess)]
    pub claimant_wallet: UncheckedAccount<'info>,
    pub claimant: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<ClaimTournamentPrize>, tournament_id: u64) -> Result<()> {
    let tournament = &mut ctx.accounts.tournament;
    let claimant_key = ctx.accounts.claimant.key();

    require!(
        tournament.status == TournamentStatus::Completed,
        GameErrorCode::TournamentNotCompleted
    );

    // Determine which place the claimant finished and their prize share.
    // Covers all 10 prize positions so 128-player (top 5) and 256-player (top 10)
    // tournaments can pay out every eligible winner.
    let (place_index, prize_share_bps) =
        ledger::find_place(tournament, claimant_key).ok_or(GameErrorCode::NotTournamentWinner)?;

    require!(prize_share_bps > 0, GameErrorCode::NoPrizeToClaim);

    // Prevent double-claiming using bitflags
    let place_bit = ledger::place_bit(place_index)?;
    require!(
        (tournament.prizes_claimed & place_bit) == 0,
        GameErrorCode::PrizeAlreadyClaimed
    );
    tournament.prizes_claimed |= place_bit;

    // ── USDC prize path (host-funded guaranteed pool) ─────────────────────────
    // Pays winner's % share of the USDC that the operator locked before registration.
    if tournament.usdc_prize_mint.is_some() && tournament.usdc_prize_pool > 0 {
        let usdc_prize_escrow = ctx
            .accounts
            .usdc_prize_escrow
            .as_ref()
            .ok_or(GameErrorCode::MissingTokenAccounts)?;
        let claimant_usdc_ata = ctx
            .accounts
            .claimant_usdc_ata
            .as_ref()
            .ok_or(GameErrorCode::MissingTokenAccounts)?;

        require!(
            claimant_usdc_ata.owner == claimant_key,
            GameErrorCode::UnauthorizedAccess
        );

        let usdc_prize = ledger::prize_amount(tournament.usdc_prize_pool, prize_share_bps)?;

        if usdc_prize > 0 {
            let tournament_id_bytes = tournament_id.to_le_bytes();
            let bump = ctx.bumps.usdc_prize_escrow_authority;
            let escrow_seeds: &[&[&[u8]]] =
                &[&[TOURNAMENT_USDC_PRIZE_SEED, &tournament_id_bytes, &[bump]]];
            token::transfer(
                CpiContext::new_with_signer(
                    Token::id(),
                    Transfer {
                        from: usdc_prize_escrow.to_account_info(),
                        to: claimant_usdc_ata.to_account_info(),
                        authority: ctx.accounts.usdc_prize_escrow_authority.to_account_info(),
                    },
                    escrow_seeds,
                ),
                usdc_prize,
            )?;
        }
    }

    // ── SOL prize path (operator-funded guaranteed pool) ──────────────────────
    // Pays winner's % share of the guaranteed SOL prize the operator locked in
    // escrow before registration opened (fund_sol_prize). Entry fees never enter
    // this pool. Runs whether or not there is a USDC pool — both can pay out.
    if tournament.prize_pool > 0 {
        let sol_prize = ledger::prize_amount(tournament.prize_pool, prize_share_bps)?;

        if sol_prize > 0 {
            let escrow_lamports = ctx.accounts.escrow_pda.lamports();
            require!(
                escrow_lamports >= sol_prize,
                GameErrorCode::InsufficientPrizeFunds
            );
            **ctx.accounts.escrow_pda.lamports.borrow_mut() -= sol_prize;
            **ctx.accounts.claimant_wallet.lamports.borrow_mut() += sol_prize;
        }
    }

    // Require at least one pool paid something
    require!(
        tournament.usdc_prize_pool > 0 || tournament.prize_pool > 0,
        GameErrorCode::NoPrizeToClaim
    );

    Ok(())
}
