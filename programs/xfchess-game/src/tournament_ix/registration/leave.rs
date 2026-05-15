//! Instruction allowing players to voluntarily leave a tournament before it starts and receive a refund.
//! The entry fee is refunded from the host_treasury (operator's wallet), which must co-sign.

use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(tournament_id: u64)]
pub struct LeaveTournament<'info> {
    #[account(
        mut,
        seeds = [TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        bump = tournament.bump
    )]
    pub tournament: Account<'info, Tournament>,
    #[account(mut)]
    pub player: Signer<'info>,
    /// Host treasury wallet — must sign to authorize the SOL refund.
    /// This is the operator's wallet that received the entry fee.
    #[account(
        mut,
        constraint = host_treasury.key() == tournament.host_treasury @ GameErrorCode::UnauthorizedAccess
    )]
    pub host_treasury: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<LeaveTournament>, tournament_id: u64) -> Result<()> {
    let tournament = &mut ctx.accounts.tournament;
    let player_key = ctx.accounts.player.key();

    // Validate tournament state
    require!(
        tournament.status == TournamentStatus::Registration,
        GameErrorCode::InvalidTournamentStatus
    );

    // Find the player's index
    let mut player_index = None;
    for i in 0..tournament.num_registered_players as usize {
        if tournament.players[i] == player_key {
            player_index = Some(i);
            break;
        }
    }

    let index = player_index.ok_or(GameErrorCode::PlayerNotFound)?;

    // Remove player and their ELO by shifting the array left
    let num_players = tournament.num_registered_players as usize;
    for i in index..(num_players - 1) {
        tournament.players[i] = tournament.players[i + 1];
        tournament.player_elos[i] = tournament.player_elos[i + 1];
    }
    
    // Clear the last element (optional but clean)
    tournament.players[num_players - 1] = Pubkey::default();
    tournament.player_elos[num_players - 1] = 0;

    // Decrement the player count
    tournament.num_registered_players -= 1;

    // Refund entry fee from host_treasury
    let refund_amount = tournament.entry_fee;
    if refund_amount > 0 {
        require!(
            ctx.accounts.host_treasury.lamports() >= refund_amount,
            GameErrorCode::InsufficientTreasuryForRefund
        );

        anchor_lang::system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.host_treasury.to_account_info(),
                    to: ctx.accounts.player.to_account_info(),
                },
            ),
            refund_amount,
        )?;
    }

    // Update prize pool
    if tournament.prize_pool >= tournament.entry_fee {
        tournament.prize_pool -= tournament.entry_fee;
    }

    Ok(())
}
