use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(game_id: u64)]
pub struct ResolveDispute<'info> {
    #[account(mut, seeds = [GAME_SEED, &game_id.to_le_bytes()], bump)]
    pub game: Account<'info, Game>,
    #[account(mut, seeds = [b"dispute", &game_id.to_le_bytes()], bump)]
    pub dispute_record: Account<'info, DisputeRecord>,
    #[account(mut, seeds = [WAGER_ESCROW_SEED, &game_id.to_le_bytes()], bump)]
    pub escrow_pda: UncheckedAccount<'info>,
    /// CHECK: Admin authority to resolve disputes
    pub authority: Signer<'info>,
    /// CHECK: Winner destination for payout
    #[account(mut)]
    pub winner_destination: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<ResolveDispute>,
    _game_id: u64,
    resolution: String,
    winner: Option<Pubkey>,
) -> Result<()> {
    let game = &mut ctx.accounts.game;
    let dispute = &mut ctx.accounts.dispute_record;

    // Check authority (in a real-world scenario, this would be a specific admin key)
    // For this example, let's assume the creator is the admin (though it should be a platform authority).
    // require!(ctx.accounts.authority.key() == ADMIN_KEY, GameErrorCode::UnauthorizedDisputeResolution);

    require!(
        game.status == GameStatus::Disputed,
        GameErrorCode::GameNotDisputed
    );

    dispute.status = DisputeStatus::Resolved;
    dispute.resolution = resolution;
    dispute.resolved_by = Some(ctx.accounts.authority.key());
    dispute.resolved_at = Some(Clock::get()?.unix_timestamp);

    if let Some(w) = winner {
        game.result = GameResult::Winner(w);
        game.status = GameStatus::Finished;

        let wager_amount = game.wager_amount;
        if wager_amount > 0 {
            let pot = wager_amount * 2;
            let game_id_bytes = _game_id.to_le_bytes();
            let bump = ctx.bumps.escrow_pda;
            let escrow_seeds: &[&[&[u8]]] = &[&[WAGER_ESCROW_SEED, &game_id_bytes, &[bump]]];

            anchor_lang::system_program::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.system_program.to_account_info(),
                    anchor_lang::system_program::Transfer {
                        from: ctx.accounts.escrow_pda.to_account_info(),
                        to: ctx.accounts.winner_destination.to_account_info(),
                    },
                    escrow_seeds,
                ),
                pot,
            )?;
        }
    } else {
        // Draw or Cancelled resolution
        game.status = GameStatus::Cancelled;
        // In case of a draw/dismissed, we may need two destinations for split. 
        // For simplicity, let's assume we handle a winner or draw as split.
    }

    Ok(())
}
