#[cfg(feature = "magicblock")]
pub use self::inner::*;

#[cfg(feature = "magicblock")]
mod inner {
    use crate::errors::XfchessGameError;
    use crate::state::game::Game;
    use anchor_lang::prelude::*;
    use ephemeral_rollups_sdk::consts::DELEGATION_PROGRAM_ID;
    use ephemeral_rollups_sdk::cpi::{commit_and_undelegate_accounts, delegate_account};

    /// Delegate the Game PDA to the MagicBlock ephemeral rollup so that
    /// subsequent moves can be processed with sub-second latency on the ER.
    /// The payer (white or black) authorises the delegation.
    pub fn handler_delegate_game(
        ctx: Context<DelegateGameCtx>,
        game_id: u64,
        valid_until: i64,
    ) -> Result<()> {
        let game = &ctx.accounts.game;

        require!(
            ctx.accounts.payer.key() == game.white || ctx.accounts.payer.key() == game.black,
            XfchessGameError::UnauthorizedAccess
        );

        let seeds: Vec<Vec<u8>> = vec![
            b"game".to_vec(),
            game_id.to_le_bytes().to_vec(),
            vec![game.bump],
        ];

        delegate_account(
            &ctx.accounts.payer.to_account_info(),
            &ctx.accounts.game.to_account_info(),
            &ctx.accounts.owner_program.to_account_info(),
            &ctx.accounts.buffer.to_account_info(),
            &ctx.accounts.delegation_record.to_account_info(),
            &ctx.accounts.delegation_metadata.to_account_info(),
            &ctx.accounts.delegation_program.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
            &seeds,
            valid_until,
            300_000,
        )?;

        Ok(())
    }

    /// Commit the current ER state for the Game PDA back to the base layer
    /// and undelegate the account so it can be used on mainnet/devnet again.
    pub fn handler_undelegate_game(ctx: Context<UndelegateGameCtx>, game_id: u64) -> Result<()> {
        let game = &ctx.accounts.game;

        require!(
            ctx.accounts.payer.key() == game.white || ctx.accounts.payer.key() == game.black,
            XfchessGameError::UnauthorizedAccess
        );

        let seeds: Vec<Vec<u8>> = vec![
            b"game".to_vec(),
            game_id.to_le_bytes().to_vec(),
            vec![game.bump],
        ];

        commit_and_undelegate_accounts(
            &ctx.accounts.payer.to_account_info(),
            vec![&ctx.accounts.game.to_account_info()],
            &ctx.accounts.magic_context.to_account_info(),
            &ctx.accounts.magic_program.to_account_info(),
            &seeds,
        )?;

        Ok(())
    }

    #[derive(Accounts)]
    #[instruction(game_id: u64)]
    pub struct DelegateGameCtx<'info> {
        #[account(
            mut,
            seeds = [b"game", game_id.to_le_bytes().as_ref()],
            bump = game.bump,
        )]
        pub game: Account<'info, Game>,

        #[account(mut)]
        pub payer: Signer<'info>,

        /// CHECK: The xfchess-game program itself (owner).
        pub owner_program: AccountInfo<'info>,

        /// CHECK: Temporary buffer used during delegation (MagicBlock protocol).
        #[account(mut)]
        pub buffer: AccountInfo<'info>,

        /// CHECK: Delegation record PDA managed by the delegation program.
        #[account(mut)]
        pub delegation_record: AccountInfo<'info>,

        /// CHECK: Delegation metadata PDA managed by the delegation program.
        #[account(mut)]
        pub delegation_metadata: AccountInfo<'info>,

        /// CHECK: MagicBlock delegation program.
        #[account(address = DELEGATION_PROGRAM_ID)]
        pub delegation_program: AccountInfo<'info>,

        pub system_program: Program<'info, System>,
    }

    #[derive(Accounts)]
    #[instruction(game_id: u64)]
    pub struct UndelegateGameCtx<'info> {
        #[account(
            mut,
            seeds = [b"game", game_id.to_le_bytes().as_ref()],
            bump = game.bump,
        )]
        pub game: Account<'info, Game>,

        #[account(mut)]
        pub payer: Signer<'info>,

        /// CHECK: MagicBlock magic context account for commit/undelegate.
        #[account(mut)]
        pub magic_context: AccountInfo<'info>,

        /// CHECK: MagicBlock magic program.
        pub magic_program: AccountInfo<'info>,
    }
}
