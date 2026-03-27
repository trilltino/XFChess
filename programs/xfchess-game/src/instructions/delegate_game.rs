pub use self::inner::*;

mod inner {
    use crate::constants::MOVE_LOG_SEED;
    use crate::state::game::Game;
    use crate::state::move_log::MoveLog;
    use anchor_lang::prelude::*;
    use ephemeral_rollups_sdk::consts::DELEGATION_PROGRAM_ID;
    use ephemeral_rollups_sdk::cpi::{delegate_account, DelegateAccounts, DelegateConfig};
    use ephemeral_rollups_sdk::ephem::deprecated::v0::commit_and_undelegate_accounts;

    /// Delegate the Game PDA to the MagicBlock ephemeral rollup so that
    /// subsequent moves can be processed with sub-second latency on the ER.
    /// The payer (white or black) authorises the delegation.
    pub fn handler_delegate_game(
        ctx: Context<DelegateGameCtx>,
        _game_id: u64,
        valid_until: i64,
    ) -> Result<()> {
        // Seeds WITHOUT bump — delegate_account adds the bump internally
        let game_id_bytes = _game_id.to_le_bytes();
        let seeds: &[&[u8]] = &[b"game", &game_id_bytes];

        // EU devnet validator for devnet-eu.magicblock.app
        let eu_validator = "MEUGGrYPxKk17hCr7wpT6s8dtNokZj5U2L57vjYMS8e"
            .parse::<Pubkey>()
            .unwrap();
        let config = DelegateConfig {
            commit_frequency_ms: (valid_until as u32).saturating_mul(1000),
            validator: Some(eu_validator),
        };

        // Delegate the game PDA
        let delegate_accounts = DelegateAccounts {
            payer: &ctx.accounts.payer.to_account_info(),
            pda: &ctx.accounts.game.to_account_info(),
            owner_program: &ctx.accounts.owner_program.to_account_info(),
            buffer: &ctx.accounts.buffer.to_account_info(),
            delegation_record: &ctx.accounts.delegation_record.to_account_info(),
            delegation_metadata: &ctx.accounts.delegation_metadata.to_account_info(),
            delegation_program: &ctx.accounts.delegation_program.to_account_info(),
            system_program: &ctx.accounts.system_program.to_account_info(),
        };

        delegate_account(delegate_accounts, seeds, config.clone())?;

        // Delegate the move_log PDA
        let ml_seeds: &[&[u8]] = &[MOVE_LOG_SEED, &game_id_bytes];

        let ml_delegate_accounts = DelegateAccounts {
            payer: &ctx.accounts.payer.to_account_info(),
            pda: &ctx.accounts.move_log.to_account_info(),
            owner_program: &ctx.accounts.owner_program.to_account_info(),
            buffer: &ctx.accounts.ml_buffer.to_account_info(),
            delegation_record: &ctx.accounts.ml_delegation_record.to_account_info(),
            delegation_metadata: &ctx.accounts.ml_delegation_metadata.to_account_info(),
            delegation_program: &ctx.accounts.delegation_program.to_account_info(),
            system_program: &ctx.accounts.system_program.to_account_info(),
        };

        delegate_account(ml_delegate_accounts, ml_seeds, config)?;

        Ok(())
    }

    /// Commit the current ER state for the Game PDA back to the base layer
    /// and undelegate the account so it can be used on mainnet/devnet again.
    /// No payer identity check — the VPS session key may trigger this so no
    /// extra wallet popup is needed at game end.
    pub fn handler_undelegate_game(ctx: Context<UndelegateGameCtx>, _game_id: u64) -> Result<()> {
        commit_and_undelegate_accounts(
            &ctx.accounts.payer.to_account_info(),
            vec![
                &ctx.accounts.game.to_account_info(),
                &ctx.accounts.move_log.to_account_info(),
            ],
            &ctx.accounts.magic_context.to_account_info(),
            &ctx.accounts.magic_program.to_account_info(),
        )?;

        Ok(())
    }

    #[derive(Accounts)]
    #[instruction(_game_id: u64)]
    pub struct DelegateGameCtx<'info> {
        #[account(
            mut,
            seeds = [b"game", _game_id.to_le_bytes().as_ref()],
            bump = game.bump,
        )]
        pub game: Account<'info, Game>,

        #[account(
            mut,
            seeds = [MOVE_LOG_SEED, _game_id.to_le_bytes().as_ref()],
            bump,
        )]
        pub move_log: Account<'info, MoveLog>,

        #[account(mut)]
        pub payer: Signer<'info>,

        /// CHECK: The xfchess-game program itself (owner).
        pub owner_program: AccountInfo<'info>,

        /// CHECK: Temporary buffer for game PDA delegation.
        #[account(mut)]
        pub buffer: AccountInfo<'info>,

        /// CHECK: Delegation record for game PDA.
        #[account(mut)]
        pub delegation_record: AccountInfo<'info>,

        /// CHECK: Delegation metadata for game PDA.
        #[account(mut)]
        pub delegation_metadata: AccountInfo<'info>,

        /// CHECK: Temporary buffer for move_log PDA delegation.
        #[account(mut)]
        pub ml_buffer: AccountInfo<'info>,

        /// CHECK: Delegation record for move_log PDA.
        #[account(mut)]
        pub ml_delegation_record: AccountInfo<'info>,

        /// CHECK: Delegation metadata for move_log PDA.
        #[account(mut)]
        pub ml_delegation_metadata: AccountInfo<'info>,

        /// CHECK: MagicBlock delegation program.
        #[account(address = ephemeral_rollups_sdk::id())]
        pub delegation_program: AccountInfo<'info>,

        pub system_program: Program<'info, System>,
    }

    #[derive(Accounts)]
    #[instruction(_game_id: u64)]
    pub struct UndelegateGameCtx<'info> {
        #[account(
            mut,
            seeds = [b"game", _game_id.to_le_bytes().as_ref()],
            bump = game.bump,
        )]
        pub game: Account<'info, Game>,

        #[account(
            mut,
            seeds = [MOVE_LOG_SEED, _game_id.to_le_bytes().as_ref()],
            bump,
        )]
        pub move_log: Account<'info, MoveLog>,

        #[account(mut)]
        pub payer: Signer<'info>,

        /// CHECK: MagicBlock magic context account for commit/undelegate.
        #[account(mut)]
        pub magic_context: AccountInfo<'info>,

        /// CHECK: MagicBlock magic program.
        pub magic_program: AccountInfo<'info>,
    }
}
