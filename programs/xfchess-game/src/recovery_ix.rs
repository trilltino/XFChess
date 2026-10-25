//! Versioned lifecycle instructions preserve legacy account prefixes and append
//! the permanent recovery record. Legacy empty-Game recovery stays fail-closed.
use crate::delegation_ix::delegate::{
    __client_accounts_delegate_game_ctx, __cpi_client_accounts_delegate_game_ctx, DelegateGameCtx,
    DelegateGameCtxBumps,
};
use crate::game_ix::create::{
    __client_accounts_create_game, __cpi_client_accounts_create_game, CreateGame, CreateGameBumps,
};
use crate::game_ix::global_create::{
    __client_accounts_global_create_game, __cpi_client_accounts_global_create_game,
    GlobalCreateGame, GlobalCreateGameBumps,
};
use crate::game_ix::global_join::{
    __client_accounts_global_join_game, __cpi_client_accounts_global_join_game, GlobalJoinGame,
    GlobalJoinGameBumps,
};
use crate::game_ix::join::{
    __client_accounts_join_game, __cpi_client_accounts_join_game, JoinGame, JoinGameBumps,
};
use crate::governance_ix::recover_stuck_delegation::{
    __client_accounts_recover_stuck_delegation, __cpi_client_accounts_recover_stuck_delegation,
    RecoverStuckDelegation, RecoverStuckDelegationBumps,
};
use crate::tournament_ix::matches::create_tournament_game::{
    __client_accounts_create_tournament_game, __cpi_client_accounts_create_tournament_game,
    CreateTournamentGame, CreateTournamentGameBumps,
};
use crate::tournament_ix::session::session_create_game::{
    __client_accounts_session_create_game, __cpi_client_accounts_session_create_game,
    SessionCreateGame, SessionCreateGameBumps,
};
use crate::tournament_ix::session::session_join_game::{
    __client_accounts_session_join_game, __cpi_client_accounts_session_join_game, SessionJoinGame,
    SessionJoinGameBumps,
};
use crate::{errors::GameErrorCode, state::*};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::{program::invoke_signed, system_instruction};

#[derive(Accounts)]
pub struct CreateGameV2<'info> {
    pub legacy: CreateGame<'info>,
    /// CHECK: canonical, fresh System-owned PDA; initialized manually after creation.
    #[account(mut)]
    pub recovery_identity: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct GlobalCreateGameV2<'info> {
    pub legacy: GlobalCreateGame<'info>,
    /// CHECK: canonical, fresh PDA, funded only from the validated session vault.
    #[account(mut)]
    pub recovery_identity: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct JoinGameV2<'info> {
    pub legacy: JoinGame<'info>,
    #[account(mut)]
    pub recovery_identity: Account<'info, RecoveryIdentity>,
}

#[derive(Accounts)]
pub struct GlobalJoinGameV2<'info> {
    pub legacy: GlobalJoinGame<'info>,
    #[account(mut)]
    pub recovery_identity: Account<'info, RecoveryIdentity>,
}

#[derive(Accounts)]
pub struct CreateTournamentGameV2<'info> {
    pub legacy: CreateTournamentGame<'info>,
    /// CHECK: canonical fresh PDA; original handler authenticates tournament seats.
    #[account(mut)]
    pub recovery_identity: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct SessionCreateGameV2<'info> {
    pub legacy: SessionCreateGame<'info>,
    /// CHECK: canonical fresh PDA; funded from the validated tournament session vault.
    #[account(mut)]
    pub recovery_identity: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct SessionJoinGameV2<'info> {
    pub legacy: SessionJoinGame<'info>,
    #[account(mut)]
    pub recovery_identity: Account<'info, RecoveryIdentity>,
}

#[derive(Accounts)]
pub struct DelegateGameV2<'info> {
    pub legacy: DelegateGameCtx<'info>,
    #[account(mut)]
    pub recovery_identity: Account<'info, RecoveryIdentity>,
}

#[derive(Accounts)]
pub struct RecoverStuckDelegationV2<'info> {
    pub legacy: RecoverStuckDelegation<'info>,
    #[account(mut)]
    pub recovery_identity: Account<'info, RecoveryIdentity>,
}

/// Payer is selected by each authenticated wrapper, never by arbitrary remaining accounts.
pub fn require_identity_pda(info: &AccountInfo<'_>, game_id: u64) -> Result<u8> {
    let (expected, bump) = Pubkey::find_program_address(
        &[RecoveryIdentity::SEED, &game_id.to_le_bytes()],
        &crate::ID,
    );
    require_keys_eq!(*info.key, expected, GameErrorCode::RecoveryIdentityMismatch);
    Ok(bump)
}

/// Legacy creation/join cannot reuse or bypass a live or terminal v2 binding.
pub fn require_legacy_identity_absent(accounts: &[AccountInfo<'_>], game_id: u64) -> Result<()> {
    let record = accounts
        .first()
        .ok_or(GameErrorCode::RecoveryIdentityUnavailable)?;
    require_identity_pda(record, game_id)?;
    require!(
        record.owner == &System::id() && record.data_is_empty(),
        GameErrorCode::RecoveryIdentityMismatch
    );
    Ok(())
}

pub fn initialize_identity<'info>(
    target: &AccountInfo<'info>,
    payer: &AccountInfo<'info>,
    system: &AccountInfo<'info>,
    record: &RecoveryIdentity,
    session_vault: bool,
) -> Result<()> {
    let bump = require_identity_pda(target, record.game_id)?;
    require!(record.bump == bump, GameErrorCode::RecoveryIdentityMismatch);
    require!(
        target.owner == &System::id() && target.data_is_empty(),
        GameErrorCode::RecoveryIdentityMismatch
    );
    let size = 8 + RecoveryIdentity::INIT_SPACE;
    let rent = Rent::get()?.minimum_balance(size);
    let top_up = rent.saturating_sub(target.lamports());
    if session_vault {
        let reserve = Rent::get()?.minimum_balance(payer.data_len());
        let required = reserve
            .checked_add(top_up)
            .ok_or(GameErrorCode::ArithmeticOverflow)?;
        require!(
            payer.lamports() >= required,
            GameErrorCode::GlobalSessionVaultUnderfunded
        );
    } else if top_up > 0 {
        anchor_lang::system_program::transfer(
            CpiContext::new(
                System::id(),
                anchor_lang::system_program::Transfer {
                    from: payer.clone(),
                    to: target.clone(),
                },
            ),
            top_up,
        )?;
    }
    let id_bytes = record.game_id.to_le_bytes();
    let seeds: &[&[u8]] = &[RecoveryIdentity::SEED, &id_bytes, &[record.bump]];
    invoke_signed(
        &system_instruction::allocate(target.key, size as u64),
        &[target.clone(), system.clone()],
        &[seeds],
    )?;
    invoke_signed(
        &system_instruction::assign(target.key, &crate::ID),
        &[target.clone(), system.clone()],
        &[seeds],
    )?;
    // Complete System CPIs before changing program-owned lamports. A CPI that
    // syncs only the credited target, without its debited vault, is unbalanced.
    if session_vault {
        crate::common::escrow::debit_program_pda(payer, target, top_up)?;
    }
    record.try_serialize(&mut &mut target.try_borrow_mut_data()?[..])?;
    Ok(())
}

/// All ordinary payout instructions require this canonical trailing account.
/// A nonexistent System-owned PDA supports legacy games without fabricating data.
/// Requiring it prevents callers from omitting an existing record to bypass closure.
pub fn close_identity(accounts: &[AccountInfo<'_>], game_id: u64) -> Result<()> {
    let info = accounts
        .first()
        .ok_or(GameErrorCode::RecoveryIdentityUnavailable)?;
    let expected = Pubkey::find_program_address(
        &[RecoveryIdentity::SEED, &game_id.to_le_bytes()],
        &crate::ID,
    )
    .0;
    require_keys_eq!(*info.key, expected, GameErrorCode::RecoveryIdentityMismatch);
    if info.owner == &System::id() && info.data_is_empty() {
        return Ok(());
    }
    require!(
        info.owner == &crate::ID && info.is_writable,
        GameErrorCode::RecoveryIdentityMismatch
    );
    let mut record = RecoveryIdentity::try_deserialize(&mut &info.try_borrow_data()?[..])?;
    require!(
        record.game_id == game_id && record.status != RecoveryStatus::Recovered,
        GameErrorCode::RecoveryIdentityMismatch
    );
    record.status = RecoveryStatus::Closed;
    record.try_serialize(&mut &mut info.try_borrow_mut_data()?[..])?;
    Ok(())
}

pub fn recover(ctx: Context<RecoverStuckDelegationV2>, game_id: u64) -> Result<()> {
    require_identity_pda(&ctx.accounts.recovery_identity.to_account_info(), game_id)?;
    let accounts = &ctx.accounts.legacy;
    require!(
        accounts.game.owner == &crate::ID && accounts.game.data_is_empty(),
        GameErrorCode::GameNotStuckDelegation
    );
    let record = &mut ctx.accounts.recovery_identity;
    require!(
        record.game_id == game_id
            && record.status == RecoveryStatus::Delegated
            && record.white != Pubkey::default()
            && record.black != Pubkey::default(),
        GameErrorCode::RecoveryIdentityMismatch
    );
    require_keys_neq!(record.white, record.black, GameErrorCode::CannotPlaySelf);
    require_keys_eq!(
        accounts.white_authority.key(),
        record.white,
        GameErrorCode::RecoveryIdentityMismatch
    );
    require_keys_eq!(
        accounts.black_authority.key(),
        record.black,
        GameErrorCode::RecoveryIdentityMismatch
    );
    let share = record.wager_amount;
    let pot = share
        .checked_mul(2)
        .ok_or(GameErrorCode::ArithmeticOverflow)?;
    require!(
        accounts.escrow_pda.lamports() >= pot,
        GameErrorCode::RecoveryIdentityMismatch
    );
    if share > 0 {
        let remaining = accounts.escrow_pda.lamports() - pot;
        require!(
            remaining == 0 || Rent::get()?.is_exempt(remaining, 0),
            GameErrorCode::RecoveryIdentityMismatch
        );
        crate::common::escrow::require_rent_exempt_after(accounts.white_authority.as_ref(), share)?;
        crate::common::escrow::require_rent_exempt_after(accounts.black_authority.as_ref(), share)?;
    }
    // State and all transfers are atomic at the Solana instruction boundary.
    record.status = RecoveryStatus::Recovered;
    for recipient in [
        accounts.white_authority.as_ref(),
        accounts.black_authority.as_ref(),
    ] {
        crate::common::escrow::pay_from_game_escrow(
            &accounts.system_program,
            &accounts.escrow_pda,
            recipient,
            share,
            game_id,
            ctx.bumps.legacy.escrow_pda,
        )?;
    }
    let rent = accounts.game.lamports();
    let treasury = accounts.treasury_vault.to_account_info();
    let received = treasury
        .lamports()
        .checked_add(rent)
        .ok_or(GameErrorCode::ArithmeticOverflow)?;
    **accounts.game.try_borrow_mut_lamports()? = 0;
    **treasury.try_borrow_mut_lamports()? = received;
    emit!(crate::events::StuckDelegationRecovered {
        game_id,
        dispute_authority: accounts.dispute_authority.key(),
        white_authority: record.white,
        black_authority: record.black,
        white_share: share,
        black_share: share,
        timestamp: Clock::get()?.unix_timestamp
    });
    Ok(())
}
