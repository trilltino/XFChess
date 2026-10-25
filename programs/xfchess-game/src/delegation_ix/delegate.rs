pub use self::inner::*;

mod inner {
    use crate::errors::GameErrorCode;
    use crate::state::Game;
    use anchor_lang::prelude::*;
    use ephemeral_rollups_sdk::cpi::DelegateAccounts;

    pub fn handler_delegate_game(
        ctx: Context<DelegateGameCtx>,
        _game_id: u64,
        // Retained for instruction ABI compatibility; unused for commit cadence.
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

        crate::lifecycle::guards::require_delegated(&game_struct)?;
        crate::lifecycle::transitions::require_undelegation_authority(
            &game_struct,
            ctx.accounts.payer.key(),
        )?;
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
        /// CHECK: Canonical Game PDA; v2 validates ownership and state before the SDK changes ownership during delegation.
        pub game: UncheckedAccount<'info>,

        #[account(mut)]
        pub payer: Signer<'info>,

        #[account(address = crate::ID @ GameErrorCode::InvalidOwnerProgram)]
        /// CHECK: Address is pinned to this program; SDK uses its key as the delegated account owner.
        pub owner_program: UncheckedAccount<'info>,

        #[account(mut)]
        /// CHECK: SDK derives and creates the canonical delegation buffer with program PDA signer seeds.
        pub buffer: UncheckedAccount<'info>,

        #[account(mut)]
        /// CHECK: Delegation Program CPI validates its record; this handler neither deserializes nor directly writes it.
        pub delegation_record: UncheckedAccount<'info>,

        #[account(mut)]
        /// CHECK: Delegation Program CPI validates metadata; no direct application write or deserialization.
        pub delegation_metadata: UncheckedAccount<'info>,

        #[account(address = ephemeral_rollups_sdk::id())]
        /// CHECK: Address pinned to the SDK Delegation Program; runtime verifies the invoked program.
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
        /// CHECK: Game PDA can have delegation ownership; handler deserializes its state and authenticates the payer before commit.
        pub game: UncheckedAccount<'info>,

        #[account(mut)]
        pub payer: Signer<'info>,

        #[account(mut, address = ephemeral_rollups_sdk::consts::MAGIC_CONTEXT_ID)]
        /// CHECK: Address pinned to the SDK Magic context; forwarded to the Magic intent CPI.
        pub magic_context: UncheckedAccount<'info>,

        #[account(address = ephemeral_rollups_sdk::consts::MAGIC_PROGRAM_ID)]
        /// CHECK: Address pinned to the SDK Magic Program; used as the commit CPI target.
        pub magic_program: UncheckedAccount<'info>,
    }
}
