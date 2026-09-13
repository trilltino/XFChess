pub use self::inner::*;

mod inner {
    use crate::errors::GameErrorCode;
    use crate::state::Game;
    use anchor_lang::prelude::*;
    use ephemeral_rollups_sdk::cpi::DelegateAccounts;

    pub fn handler_delegate_game(
        ctx: Context<DelegateGameCtx>,
        _game_id: u64,
        // Retained for instruction ABI compatibility; no longer used to derive
        // the commit cadence (that was a bug — it multiplied a unix timestamp).
        _valid_until: i64,
    ) -> Result<()> {
        let game_id_bytes = _game_id.to_le_bytes();

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

        let mut game_data = ctx.accounts.game.try_borrow_mut_data()?;
        let mut game = Game::try_deserialize(&mut &game_data[..])?;

        let fee_payer = &ctx.accounts.fee_payer;

        require!(
            game.fee_payer == fee_payer.key(),
            GameErrorCode::FeePayerMismatch
        );
        crate::lifecycle::transitions::mark_delegated(&mut game)?;

        let mut writer = &mut game_data[..];
        game.try_serialize(&mut writer)?;
        drop(game_data);

        crate::magicblock::delegation::delegate_game_pda(delegate_accounts, &game_id_bytes)?;

        Ok(())
    }

    pub fn handler_undelegate_game(ctx: Context<UndelegateGameCtx>, _game_id: u64) -> Result<()> {
        let mut data = ctx.accounts.game.try_borrow_mut_data()?;
        let mut game_struct = Game::try_deserialize(&mut &data[..])?;

        crate::lifecycle::transitions::mark_undelegated(&mut game_struct)?;

        let mut writer = &mut data[..];
        game_struct.try_serialize(&mut writer)?;

        drop(data);

        crate::magicblock::delegation::commit_and_undelegate_game_pda(
            &ctx.accounts.payer.to_account_info(),
            &ctx.accounts.game.to_account_info(),
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
            bump,
        )]
        pub game: UncheckedAccount<'info>,

        #[account(mut)]
        pub payer: Signer<'info>,

        #[account(address = crate::ID @ GameErrorCode::InvalidOwnerProgram)]
        pub owner_program: UncheckedAccount<'info>,

        #[account(mut)]
        pub buffer: UncheckedAccount<'info>,

        #[account(mut)]
        pub delegation_record: UncheckedAccount<'info>,

        #[account(mut)]
        pub delegation_metadata: UncheckedAccount<'info>,

        #[account(address = ephemeral_rollups_sdk::id())]
        pub delegation_program: UncheckedAccount<'info>,

        pub system_program: Program<'info, System>,

        #[account(mut)]
        pub fee_payer: Signer<'info>,
    }

    #[derive(Accounts)]
    #[instruction(_game_id: u64)]
    pub struct UndelegateGameCtx<'info> {
        #[account(
            mut,
            seeds = [b"game", _game_id.to_le_bytes().as_ref()],
            bump,
        )]
        pub game: UncheckedAccount<'info>,

        #[account(mut)]
        pub payer: Signer<'info>,

        #[account(mut, address = ephemeral_rollups_sdk::consts::MAGIC_CONTEXT_ID)]
        pub magic_context: UncheckedAccount<'info>,

        #[account(address = ephemeral_rollups_sdk::consts::MAGIC_PROGRAM_ID)]
        pub magic_program: UncheckedAccount<'info>,
    }
}
