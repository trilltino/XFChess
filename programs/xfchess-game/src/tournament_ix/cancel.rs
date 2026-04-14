//! Instruction for safely halting a tournament and refunding entry fees.
//! Uses remaining_accounts for variable player count (up to 128).

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
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler<'a, 'b, 'c, 'info>(ctx: Context<'a, 'b, 'c, 'info, CancelTournament<'info>>, tournament_id: u64) -> Result<()> {
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

        // Use remaining_accounts for player wallets
        require!(
            ctx.remaining_accounts.len() >= registered,
            GameErrorCode::NotInGame
        );

        for i in 0..registered {
            let player_key = ctx.accounts.tournament.players[i];
            let player_wallet = &ctx.remaining_accounts[i];
            require!(
                player_wallet.key() == player_key,
                GameErrorCode::NotInGame
            );
            require!(
                player_wallet.is_writable,
                GameErrorCode::UnauthorizedAccess
            );
            anchor_lang::system_program::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.system_program.to_account_info(),
                    anchor_lang::system_program::Transfer {
                        from: ctx.accounts.escrow_pda.to_account_info(),
                        to: player_wallet.to_account_info(),
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
