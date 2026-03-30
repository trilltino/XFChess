use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(tournament_id: u64)]
pub struct ClaimTournamentPrize<'info> {
    #[account(
        mut,
        seeds = [TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        bump = tournament.bump
    )]
    pub tournament: Account<'info, Tournament>,
    /// CHECK: Escrow PDA holding prize pool.
    #[account(
        mut,
        seeds = [TOURNAMENT_ESCROW_SEED, &tournament_id.to_le_bytes()],
        bump
    )]
    pub escrow_pda: UncheckedAccount<'info>,
    /// CHECK: Winner's wallet — must match the signing winner key.
    #[account(mut, constraint = winner_wallet.key() == winner.key() @ GameErrorCode::UnauthorizedAccess)]
    pub winner_wallet: UncheckedAccount<'info>,
    pub winner: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<ClaimTournamentPrize>, tournament_id: u64) -> Result<()> {
    let tournament = &ctx.accounts.tournament;

    require!(
        tournament.status == TournamentStatus::Completed,
        GameErrorCode::TournamentNotCompleted
    );

    let winner_key = tournament.winner.ok_or(GameErrorCode::NoPrizeToClaim)?;
    require!(
        ctx.accounts.winner.key() == winner_key,
        GameErrorCode::UnauthorizedAccess
    );

    let prize = tournament.prize_pool;
    require!(prize > 0, GameErrorCode::NoPrizeToClaim);

    let tournament_id_bytes = tournament_id.to_le_bytes();
    let bump = ctx.bumps.escrow_pda;
    let escrow_seeds: &[&[&[u8]]] = &[&[TOURNAMENT_ESCROW_SEED, &tournament_id_bytes, &[bump]]];

    anchor_lang::system_program::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.system_program.to_account_info(),
            anchor_lang::system_program::Transfer {
                from: ctx.accounts.escrow_pda.to_account_info(),
                to: ctx.accounts.winner_wallet.to_account_info(),
            },
            escrow_seeds,
        ),
        prize,
    )?;

    msg!(
        "Tournament {} prize {} lamports claimed by {}",
        tournament_id,
        prize,
        winner_key
    );
    Ok(())
}
