//! Session-signed variant of `join_game` using a global persistent session key.
//!
//! The session key co-signs the join; wager funds come from the
//! [`GlobalSessionDelegation`] vault — no wallet popup for the joiner.

use crate::account_ix::session_guards;
use crate::common::escrow::debit_program_pda;
use crate::constants::{GAME_SEED, JOIN_GAME_COST, PROFILE_SEED, WAGER_ESCROW_SEED};
use crate::errors::GameErrorCode;
use crate::state::{Game, GameStatus, GameType, GlobalSessionDelegation, PlayerProfile};
use anchor_lang::prelude::*;

/// Accounts for session-signed joining. `white_profile` is read for
/// cross-border fee context, same as the plain `JoinGame` path.
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

    /// Hot key that signs on behalf of the player.
    pub session_signer: Signer<'info>,

    /// CHECK: Verified against session_delegation.player.
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

    /// CHECK: PDA for escrowing SOL wager.
    #[account(mut, seeds = [WAGER_ESCROW_SEED, &game_id.to_le_bytes()], bump)]
    pub escrow_pda: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

/// Validates the session and game state, draws the wager from the session
/// delegation vault into escrow (SOL wagers only), decrements
/// `games_remaining`, and activates the game with the player as black.
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

    // Transfer wager from delegation vault to escrow. `session_delegation` is
    // a program-owned PDA carrying real account data, so the System Program
    // refuses to act as `from` for it in any CPI ("Transfer: `from` must not
    // carry data") no matter how it's signed — `debit_program_pda` moves the
    // lamports directly instead, which is what a program is allowed to do
    // with an account it owns. See `game_ix::global_create` for the fuller
    // writeup (this is the join-side half of the same bug).
    let wager = game.wager_amount;
    if game.wager_token.is_none() {
        // Soft caps (`has_budget`, above) are not a balance check — see the
        // fuller writeup in `global_create`. Verify the vault can actually cover
        // the wager and still stay rent-exempt, so a shortfall reports as
        // "top up your session" instead of a bare insufficient-funds failure
        // from inside `debit_program_pda`. No rent term for a game account here:
        // unlike create, join does not allocate one.
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

    // Update session bookkeeping
    let session = &mut ctx.accounts.session_delegation;
    session.total_spent = session_guards::checked_session_total(session.total_spent, wager)?;
    session.games_remaining = session.games_remaining.saturating_sub(1);

    // Update game
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
