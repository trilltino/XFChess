//! Instruction for delegating games to MagicBlock Ephemeral Rollups.

pub use self::inner::*;

mod inner {
    use crate::constants::DELEGATE_COST;
    use crate::state::{Game, GameStatus};
    use crate::errors::GameErrorCode;
    use anchor_lang::prelude::*;
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
            .map_err(|_| GameErrorCode::InvalidArgument)?;
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

        // Manually deserialize, modify, and serialize game PDA state BEFORE the owner is changed by delegate_account CPI.
        let mut game_data = ctx.accounts.game.try_borrow_mut_data()?;
        let mut game = Game::try_deserialize(&mut &game_data[..])?;

        let fee_payer = &ctx.accounts.fee_payer;

        require!(game.status == GameStatus::Active, GameErrorCode::GameNotActive);
        require!(game.fee_payer == fee_payer.key(), GameErrorCode::FeePayerMismatch);

        game.fees_advanced = game.fees_advanced.checked_add(DELEGATE_COST).ok_or(GameErrorCode::ArithmeticOverflow)?;
        game.is_delegated = true;

        let mut writer = &mut game_data[..];
        game.try_serialize(&mut writer)?;
        drop(game_data);

        // Delegate the game PDA — must happen AFTER all game mutations because
        // the CPI changes the account owner to the delegation program.
        delegate_account(delegate_accounts, seeds, config)?;

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
        game_struct.is_delegated = false;

        // 3. Manually serialize it back
        let mut writer = &mut data[..];
        game_struct.try_serialize(&mut writer)?;

        // 4. Drop the data borrow before calling CPI to avoid borrow conflicts
        drop(data);

        // 5. Call delegation CPI
        commit_and_undelegate_accounts(
            &ctx.accounts.payer.to_account_info(),
            vec![
                &ctx.accounts.game.to_account_info(),
            ],
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
        #[account(mut)]
        pub magic_context: AccountInfo<'info>,

        /// CHECK: MagicBlock magic program.
        pub magic_program: AccountInfo<'info>,
    }
}
