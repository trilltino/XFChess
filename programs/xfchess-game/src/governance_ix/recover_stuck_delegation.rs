use crate::constants::*;
use crate::errors::GameErrorCode;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(game_id: u64)]
pub struct RecoverStuckDelegation<'info> {
    #[account(mut, seeds = [GAME_SEED, &game_id.to_le_bytes()], bump)]
    /// CHECK: handler verifies program ownership and an empty, recovered account.
    pub game: UncheckedAccount<'info>,
    #[account(mut, seeds = [WAGER_ESCROW_SEED, &game_id.to_le_bytes()], bump)]
    pub escrow_pda: SystemAccount<'info>,
    #[account(mut, seeds = [TREASURY_VAULT_SEED], bump)]
    pub treasury_vault: SystemAccount<'info>,
    #[account(mut)]
    pub white_authority: SystemAccount<'info>,
    #[account(mut)]
    pub black_authority: SystemAccount<'info>,
    #[account(constraint = dispute_authority.key() == crate::constants::dispute_authority::ID @ GameErrorCode::UnauthorizedDisputeResolution)]
    pub dispute_authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<RecoverStuckDelegation>, _game_id: u64) -> Result<()> {
    require_keys_eq!(
        *ctx.accounts.game.owner,
        crate::ID,
        GameErrorCode::GameNotStuckDelegation
    );
    require!(
        ctx.accounts.game.data_is_empty(),
        GameErrorCode::GameNotStuckDelegation
    );

    // Wiped Game data cannot authenticate either original player. Neither the
    // dispute authority's signature nor caller-provided recipients are proof.
    // Preserve escrow AND Game rent until an approved identity proof exists.
    err!(GameErrorCode::RecoveryIdentityUnavailable)
}
