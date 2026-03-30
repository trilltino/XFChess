use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(tournament_id: u64)]
pub struct CancelTournament<'info> {
    #[account(
        mut,
        seeds = [TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        bump = tournament.bump,
        constraint = tournament.authority == authority.key() @ GameErrorCode::NotTournamentAuthority
    )]
    pub tournament: Account<'info, Tournament>,
    /// CHECK: Escrow PDA holding collected entry fees.
    #[account(
        mut,
        seeds = [TOURNAMENT_ESCROW_SEED, &tournament_id.to_le_bytes()],
        bump
    )]
    pub escrow_pda: UncheckedAccount<'info>,
    /// CHECK: Slot-0 player wallet for refund (pass any writable wallet if slot empty).
    #[account(mut)]
    pub player0_wallet: UncheckedAccount<'info>,
    /// CHECK: Slot-1 player wallet for refund.
    #[account(mut)]
    pub player1_wallet: UncheckedAccount<'info>,
    /// CHECK: Slot-2 player wallet for refund.
    #[account(mut)]
    pub player2_wallet: UncheckedAccount<'info>,
    /// CHECK: Slot-3 player wallet for refund.
    #[account(mut)]
    pub player3_wallet: UncheckedAccount<'info>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<CancelTournament>, tournament_id: u64) -> Result<()> {
    require!(
        ctx.accounts.tournament.status == TournamentStatus::Registration
            || ctx.accounts.tournament.status == TournamentStatus::Active,
        GameErrorCode::TournamentNotActive
    );

    let refund_amount = ctx.accounts.tournament.entry_fee;
    let registered = ctx.accounts.tournament.registered_count as usize;

    if refund_amount > 0 && registered > 0 {
        let tournament_id_bytes = tournament_id.to_le_bytes();
        let bump = ctx.bumps.escrow_pda;
        let escrow_seeds: &[&[&[u8]]] =
            &[&[TOURNAMENT_ESCROW_SEED, &tournament_id_bytes, &[bump]]];

        let wallets: [&UncheckedAccount; 4] = [
            &ctx.accounts.player0_wallet,
            &ctx.accounts.player1_wallet,
            &ctx.accounts.player2_wallet,
            &ctx.accounts.player3_wallet,
        ];

        for i in 0..registered {
            require!(
                wallets[i].key() == ctx.accounts.tournament.players[i],
                GameErrorCode::NotInGame
            );
            anchor_lang::system_program::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.system_program.to_account_info(),
                    anchor_lang::system_program::Transfer {
                        from: ctx.accounts.escrow_pda.to_account_info(),
                        to: wallets[i].to_account_info(),
                    },
                    escrow_seeds,
                ),
                refund_amount,
            )?;
        }
    }

    ctx.accounts.tournament.status = TournamentStatus::Cancelled;
    msg!(
        "Tournament {} cancelled. {} players refunded {} lamports each.",
        tournament_id,
        registered,
        refund_amount
    );
    Ok(())
}
