use crate::account_ix::session_guards;
use crate::common::escrow::debit_program_pda;
use crate::constants::{GAME_SEED, JOIN_GAME_COST, PROFILE_SEED, WAGER_ESCROW_SEED};
use crate::errors::GameErrorCode;
use crate::state::{Game, GameStatus, GameType, GlobalSessionDelegation, PlayerProfile};
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(game_id: u64)]
pub struct GlobalJoinGame<'info> {
    #[account(
        mut,
        seeds = [GlobalSessionDelegation::SEED, player.key().as_ref()],
        bump = session_delegation.bump,
        constraint = session_delegation.session_key == session_signer.key() @ GameErrorCode::InvalidSessionKey,
        constraint = session_delegation.player == player.key() @ GameErrorCode::UnauthorizedAccess,
    )]
    pub session_delegation: Account<'info, GlobalSessionDelegation>,

    pub session_signer: Signer<'info>,

    pub player: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [GAME_SEED, &game_id.to_le_bytes()],
        bump = game.bump
    )]
    pub game: Account<'info, Game>,

    #[account(seeds = [PROFILE_SEED, player.key().as_ref()], bump)]
    pub player_profile: Account<'info, PlayerProfile>,

    #[account(seeds = [PROFILE_SEED, game.white.as_ref()], bump)]
    pub white_profile: Account<'info, PlayerProfile>,

    #[account(mut, seeds = [WAGER_ESCROW_SEED, &game_id.to_le_bytes()], bump)]
    pub escrow_pda: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<GlobalJoinGame>, _game_id: u64) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let session = &ctx.accounts.session_delegation;
    let game = &ctx.accounts.game;

    require!(
        session.is_valid(now),
        GameErrorCode::SessionExpiredOrDisabled
    );
    require!(
        session.games_remaining > 0,
        GameErrorCode::GlobalSessionNoGamesRemaining
    );
    require!(
        game.game_type == GameType::PvP,
        GameErrorCode::GameAlreadyFull
    );
    require!(
        game.status == GameStatus::WaitingForOpponent,
        GameErrorCode::GameAlreadyFull
    );
    require!(
        game.white != ctx.accounts.player.key(),
        GameErrorCode::CannotPlaySelf
    );
    require!(
        session.has_budget(game.wager_amount),
        GameErrorCode::GlobalSessionSpendingLimitExceeded
    );

    let wager = game.wager_amount;
    if game.wager_token.is_none() {
        let vault = ctx.accounts.session_delegation.to_account_info();
        let rent_min = Rent::get()?.minimum_balance(vault.data_len());
        let required = wager
            .checked_add(rent_min)
            .ok_or(GameErrorCode::ArithmeticOverflow)?;
        require!(
            vault.lamports() >= required,
            GameErrorCode::GlobalSessionVaultUnderfunded
        );

        debit_program_pda(
            &ctx.accounts.session_delegation.to_account_info(),
            &ctx.accounts.escrow_pda.to_account_info(),
            wager,
        )?;
    }

    let session = &mut ctx.accounts.session_delegation;
    session.total_spent = session_guards::checked_session_total(session.total_spent, wager)?;
    session.games_remaining = session.games_remaining.saturating_sub(1);

    let game = &mut ctx.accounts.game;
    game.black = ctx.accounts.player.key();
    game.status = GameStatus::Active;
    // country_fee was set at creation time from live SOL/GBP rate — no recalculation needed.
    game.fees_advanced = game
        .fees_advanced
        .checked_add(JOIN_GAME_COST)
        .ok_or(GameErrorCode::ArithmeticOverflow)?;
    game.last_move_timestamp = now;
    game.updated_at = now;

    Ok(())
}
