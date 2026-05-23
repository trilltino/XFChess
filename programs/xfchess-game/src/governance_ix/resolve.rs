//! Instruction for admins to resolve an open dispute and allocate the prize pool.

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
    pub dispute: Account<'info, DisputeRecord>,
    /// CHECK: Escrow PDA validated by seeds
    #[account(mut, seeds = [WAGER_ESCROW_SEED, &game_id.to_le_bytes()], bump)]
    pub escrow_pda: UncheckedAccount<'info>,
    /// Dispute-resolution authority — only the platform key may resolve.
    #[account(address = crate::constants::dispute_authority::ID @ GameErrorCode::UnauthorizedDisputeResolution)]
    pub dispute_authority: Signer<'info>,
    /// CHECK: White player destination — must match game.white
    #[account(mut, constraint = white_account.key() == game.white @ GameErrorCode::NotInGame)]
    pub white_account: UncheckedAccount<'info>,
    /// CHECK: Black player destination — must match game.black
    #[account(mut, constraint = black_account.key() == game.black @ GameErrorCode::NotInGame)]
    pub black_account: UncheckedAccount<'info>,
    /// CHECK: Winner destination — must match winner
    #[account(mut)]
    pub winner_account: UncheckedAccount<'info>,
    /// CHECK: Platform treasury
    #[account(mut)]
    pub platform_treasury: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<ResolveDispute>,
    game_id: u64,
    resolution: String,
    winner: Option<Pubkey>,
) -> Result<()> {
    let dispute = &mut ctx.accounts.dispute;
    let game = &mut ctx.accounts.game;

    // Validate authority
    require!(
        ctx.accounts.dispute_authority.key() == crate::constants::dispute_authority::ID,
        GameErrorCode::UnauthorizedDisputeResolution
    );

    // Validate dispute state
    require!(
        dispute.status == DisputeStatus::Pending,
        GameErrorCode::GameNotDisputed
    );
    require!(dispute.game_id == game_id, GameErrorCode::InvalidGameStatus);

    // Update dispute
    dispute.status = DisputeStatus::Resolved;
    dispute.resolution = resolution;
    dispute.resolved_at = Some(Clock::get()?.unix_timestamp);

    // Update game state based on resolution
    if let Some(winner_key) = winner {
        require!(
            winner_key == game.white || winner_key == game.black,
            GameErrorCode::InvalidWinner
        );
        game.status = GameStatus::Finished;
        game.result = GameResult::Winner(winner_key);
    } else {
        // Draw or other resolution
        game.status = GameStatus::Finished;
        game.result = GameResult::Draw;
    }

    // Transfer wager if applicable
    if game.wager_amount > 0 {
        let wager_total = game.wager_amount * 2;
        // Flat infrastructure fee — not a percentage rake on the pot.
        let platform_fee = DISPUTE_RESOLUTION_COST_LAMPORTS.min(wager_total);
        let distributable = wager_total - platform_fee;

        if winner.is_some() {
            **ctx.accounts.escrow_pda.lamports.borrow_mut() -= wager_total;
            **ctx.accounts.winner_account.lamports.borrow_mut() += distributable;
            **ctx.accounts.platform_treasury.lamports.borrow_mut() += platform_fee;
        } else {
            // Draw: split distributable equally between players
            let split_amount = distributable / 2;
            **ctx.accounts.escrow_pda.lamports.borrow_mut() -= wager_total;
            **ctx.accounts.white_account.lamports.borrow_mut() += split_amount;
            **ctx.accounts.black_account.lamports.borrow_mut() += split_amount;
            **ctx.accounts.platform_treasury.lamports.borrow_mut() += platform_fee;
        }
    }


    Ok(())
}
