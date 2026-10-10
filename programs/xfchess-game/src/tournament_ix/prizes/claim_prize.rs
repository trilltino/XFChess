use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use crate::tournament_ix::lifecycle::initialize_escrow::TournamentEscrow;
use crate::tournament_ix::prizes::ledger;
use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, TransferChecked};

#[derive(Accounts)]
#[instruction(tournament_id: u64)]
pub struct ClaimTournamentPrize<'info> {
    #[account(
        mut,
        seeds = [TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        bump = tournament.bump
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
    pub usdc_prize_escrow: Option<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub claimant_usdc_ata: Option<Account<'info, TokenAccount>>,
    pub usdc_mint: Option<Account<'info, token::Mint>>,
    #[account(
        mut,
        seeds = [TOURNAMENT_ESCROW_SEED, &tournament_id.to_le_bytes()],
        bump
    )]
    pub escrow_pda: Account<'info, TournamentEscrow>,
    #[account(
        mut,
        constraint = claimant_wallet.key() == claimant.key() @ GameErrorCode::UnauthorizedAccess,
        constraint = claimant_wallet.owner == &system_program::ID @ GameErrorCode::InvalidAccountOwner
    )]
    pub claimant_wallet: UncheckedAccount<'info>,
    pub claimant: Signer<'info>,
    #[account(address = token::ID @ GameErrorCode::UnsupportedMintExtension)]
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

    let (place_index, prize_share_bps) =
        ledger::find_place(tournament, claimant_key).ok_or(GameErrorCode::NotTournamentWinner)?;

    require!(prize_share_bps > 0, GameErrorCode::NoPrizeToClaim);

    let place_bit = ledger::place_bit(place_index)?;
    require!(
        (tournament.prizes_claimed & place_bit) == 0,
        GameErrorCode::PrizeAlreadyClaimed
    );
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
        let usdc_mint = ctx
            .accounts
            .usdc_mint
            .as_ref()
            .ok_or(GameErrorCode::MissingTokenAccounts)?;

        require_keys_eq!(
            usdc_mint.key(),
            tournament
                .usdc_prize_mint
                .ok_or(GameErrorCode::InvalidMint)?,
            GameErrorCode::InvalidMint
        );

        require!(
            claimant_usdc_ata.owner == claimant_key,
            GameErrorCode::UnauthorizedAccess
        );
        require_keys_eq!(
            claimant_usdc_ata.mint,
            usdc_mint.key(),
            GameErrorCode::InvalidMint
        );

        let usdc_prize = ledger::prize_amount(tournament.usdc_prize_pool, prize_share_bps)?;

        if usdc_prize > 0 {
            let tournament_id_bytes = tournament_id.to_le_bytes();
            let bump = ctx.bumps.usdc_prize_escrow_authority;
            let escrow_seeds: &[&[&[u8]]] =
                &[&[TOURNAMENT_USDC_PRIZE_SEED, &tournament_id_bytes, &[bump]]];
            token::transfer_checked(
                CpiContext::new_with_signer(
                    Token::id(),
                    TransferChecked {
                        from: usdc_prize_escrow.to_account_info(),
                        mint: usdc_mint.to_account_info(),
                        to: claimant_usdc_ata.to_account_info(),
                        authority: ctx.accounts.usdc_prize_escrow_authority.to_account_info(),
                    },
                    escrow_seeds,
                ),
                usdc_prize,
                usdc_mint.decimals,
            )?;
        }
    }

    // Pay the winner’s guaranteed SOL share independently of USDC prizes.
    // Entry fees do not fund this pool.
    if tournament.prize_pool > 0 {
        let sol_prize = ledger::prize_amount(tournament.prize_pool, prize_share_bps)?;

        if sol_prize > 0 {
            crate::common::escrow::debit_program_pda(
                &ctx.accounts.escrow_pda.to_account_info(),
                &ctx.accounts.claimant_wallet.to_account_info(),
                sol_prize,
            )
            .map_err(|_| GameErrorCode::InsufficientPrizeFunds)?;
        }
    }

    require!(
        tournament.usdc_prize_pool > 0 || tournament.prize_pool > 0,
        GameErrorCode::NoPrizeToClaim
    );

    tournament.prizes_claimed |= place_bit;

    Ok(())
}
