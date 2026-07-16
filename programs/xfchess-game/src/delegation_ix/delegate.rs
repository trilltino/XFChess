//! Instruction for delegating games to MagicBlock Ephemeral Rollups.

pub use self::inner::*;

mod inner {
    use crate::errors::GameErrorCode;
    use crate::state::Game;
    use anchor_lang::prelude::*;
    use ephemeral_rollups_sdk::cpi::DelegateAccounts;

    /// Delegate the Game PDA to the MagicBlock ephemeral rollup so that
    /// subsequent moves can be processed with sub-second latency on the ER.
    /// The payer (white or black) authorises the delegation.
    pub fn handler_delegate_game(
        ctx: Context<DelegateGameCtx>,
        _game_id: u64,
        // Retained for instruction ABI compatibility; no longer used to derive
        // the commit cadence (that was a bug — it multiplied a unix timestamp).
        _valid_until: i64,
    ) -> Result<()> {
        // Seeds WITHOUT bump — delegate_account adds the bump internally
        let game_id_bytes = _game_id.to_le_bytes();

        // validator: None lets the delegation program / magic-router assign a
        // validator. Pinning a single devnet pubkey here broke mainnet and forced
        // every game onto one region. See MAGICBLOCK.md at the repo root.
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

        // Manually deserialize, modify, and serialize game PDA state BEFORE the owner is changed by delegate_account CPI.
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

        // Delegate the game PDA — must happen AFTER all game mutations because
        // the CPI changes the account owner to the delegation program.
        crate::magicblock::delegation::delegate_game_pda(delegate_accounts, &game_id_bytes)?;

        Ok(())
    }

    /// Commit the current ER state for the Game PDA back to the base layer
    /// and undelegate the account so it can be used on mainnet/devnet again.
    /// No payer identity check — the VPS session key may trigger this so no
    /// extra wallet popup is needed at game end.
    pub fn handler_undelegate_game(ctx: Context<UndelegateGameCtx>, _game_id: u64) -> Result<()> {
        // 1. Manually deserialize the Game account
        let mut data = ctx.accounts.game.try_borrow_mut_data()?;
        let mut game_struct = Game::try_deserialize(&mut &data[..])?;

        // 2. Modify is_delegated
        crate::lifecycle::transitions::mark_undelegated(&mut game_struct)?;

        // 3. Manually serialize it back
        let mut writer = &mut data[..];
        game_struct.try_serialize(&mut writer)?;

        // 4. Drop the data borrow before calling CPI to avoid borrow conflicts
        drop(data);

        // 5. Call delegation CPI
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
        /// CHECK: Manual serialization is used to modify the account and serialize it back before its owner transitions.
        #[account(
            mut,
            seeds = [b"game", _game_id.to_le_bytes().as_ref()],
            bump,
        )]
        pub game: AccountInfo<'info>,

        #[account(mut)]
        pub payer: Signer<'info>,

        /// CHECK: The xfchess-game program itself (owner).
        #[account(address = crate::ID @ GameErrorCode::InvalidOwnerProgram)]
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

        /// CHECK: MagicBlock delegation program.
        #[account(address = ephemeral_rollups_sdk::id())]
        pub delegation_program: AccountInfo<'info>,

        pub system_program: Program<'info, System>,

        #[account(mut)]
        pub fee_payer: Signer<'info>,
    }

    #[derive(Accounts)]
    #[instruction(_game_id: u64)]
    pub struct UndelegateGameCtx<'info> {
        /// CHECK: Manual serialization to avoid Anchor exit serialization conflicts with delegation CPI.
        #[account(
            mut,
            seeds = [b"game", _game_id.to_le_bytes().as_ref()],
            bump,
        )]
        pub game: AccountInfo<'info>,

        #[account(mut)]
        pub payer: Signer<'info>,

        /// CHECK: MagicBlock magic context account for commit/undelegate.
        #[account(mut, address = ephemeral_rollups_sdk::consts::MAGIC_CONTEXT_ID)]
        pub magic_context: AccountInfo<'info>,

        /// CHECK: MagicBlock magic program.
        #[account(address = ephemeral_rollups_sdk::consts::MAGIC_PROGRAM_ID)]
        pub magic_program: AccountInfo<'info>,
    }
}
