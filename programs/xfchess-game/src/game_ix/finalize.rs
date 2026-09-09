use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(game_id: u64)]
pub struct EndGame<'info> {
    // Rent is returned to the recorded relayer (game.fee_payer), not to whoever
    // happens to call finalize — `close` + the matching constraint block the
    // rent/fee theft that an unconstrained destination allowed.
    #[account(
        mut,
        close = fee_payer,
        seeds = [GAME_SEED, &game_id.to_le_bytes()],
        bump,
    )]
    pub game: Account<'info, Game>,
    #[account(mut, seeds = [PROFILE_SEED, game.white.as_ref()], bump)]
    pub white_profile: Account<'info, PlayerProfile>,
    #[account(mut, seeds = [PROFILE_SEED, game.black.as_ref()], bump)]
    pub black_profile: Account<'info, PlayerProfile>,
    #[account(mut, constraint = white_authority.key() == game.white @ GameErrorCode::UnauthorizedAccess)]
    pub white_authority: SystemAccount<'info>,
    #[account(mut, constraint = black_authority.key() == game.black @ GameErrorCode::UnauthorizedAccess)]
    pub black_authority: SystemAccount<'info>,
    #[account(mut, seeds = [WAGER_ESCROW_SEED, &game_id.to_le_bytes()], bump)]
    pub escrow_pda: SystemAccount<'info>,
    #[account(mut, seeds = [TREASURY_VAULT_SEED], bump)]
    pub treasury_vault: SystemAccount<'info>,
    #[account(mut, constraint = fee_payer.key() == game.fee_payer @ GameErrorCode::FeePayerMismatch)]
    pub fee_payer: SystemAccount<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<EndGame>, game_id: u64) -> Result<()> {
    match ctx.accounts.game.status {
        GameStatus::Finished => crate::lifecycle::settlement::settle_finished_game(ctx, game_id),
        GameStatus::Cancelled => crate::lifecycle::settlement::settle_cancelled_game(ctx, game_id),
        _ => Err(GameErrorCode::GameNotFinished.into()),
    }
}
