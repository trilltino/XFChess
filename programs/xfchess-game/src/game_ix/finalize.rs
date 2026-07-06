//! Finalize a completed game through the single base-layer settlement path.

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
    /// White player wallet — must match game.white.
    #[account(mut, constraint = white_authority.key() == game.white @ GameErrorCode::UnauthorizedAccess)]
    pub white_authority: SystemAccount<'info>,
    /// Black player wallet — must match game.black.
    #[account(mut, constraint = black_authority.key() == game.black @ GameErrorCode::UnauthorizedAccess)]
    pub black_authority: SystemAccount<'info>,
    /// System-owned per-game wager escrow PDA (funds moved out via signed CPI).
    #[account(mut, seeds = [WAGER_ESCROW_SEED, &game_id.to_le_bytes()], bump)]
    pub escrow_pda: SystemAccount<'info>,
    /// Platform treasury vault — seeded PDA prevents redirection to arbitrary wallets.
    #[account(mut, seeds = [TREASURY_VAULT_SEED], bump)]
    pub treasury_vault: SystemAccount<'info>,
    /// Ephemeral-rollups relayer that funded the game — reimbursed from escrow and
    /// receives the closed game account's rent. Must be the recorded fee payer.
    #[account(mut, constraint = fee_payer.key() == game.fee_payer @ GameErrorCode::FeePayerMismatch)]
    pub fee_payer: SystemAccount<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<EndGame>, game_id: u64) -> Result<()> {
    crate::lifecycle::settlement::settle_finished_game(ctx, game_id)
}
