//! Session-signed variant of `join_game` using a global persistent session key.
//!
//! The session key co-signs the join; wager funds come from the
//! [`GlobalSessionDelegation`] vault — no wallet popup for the joiner.

use crate::constants::{GAME_SEED, PROFILE_SEED, WAGER_ESCROW_SEED, CREATE_GAME_COST};
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

pub fn handler(ctx: Context<GlobalJoinGame>, _game_id: u64) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let session = &ctx.accounts.session_delegation;
    let game = &ctx.accounts.game;

    require!(session.is_valid(now), GameErrorCode::SessionExpiredOrDisabled);
    require!(
        session.games_remaining > 0,
        GameErrorCode::GlobalSessionNoGamesRemaining
    );
    require!(game.game_type == GameType::PvP, GameErrorCode::GameAlreadyFull);
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

    // Transfer wager from delegation vault to escrow
    let wager = game.wager_amount;
    if wager > 0 && game.wager_token.is_none() {
        let player_bytes = session.player.to_bytes();
        let bump = [session.bump];
        let delegation_seeds: [&[u8]; 3] = [
            GlobalSessionDelegation::SEED,
            player_bytes.as_ref(),
            bump.as_ref(),
        ];
        let signer_seeds: &[&[&[u8]]] = &[&delegation_seeds];

        anchor_lang::system_program::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.session_delegation.to_account_info(),
                    to: ctx.accounts.escrow_pda.to_account_info(),
                },
                signer_seeds,
            ),
            wager,
        )?;
    }

    // Update session bookkeeping
    let session = &mut ctx.accounts.session_delegation;
    session.total_spent = session.total_spent.saturating_add(wager);
    session.games_remaining = session.games_remaining.saturating_sub(1);

    // Update game
    let game = &mut ctx.accounts.game;
    game.black = ctx.accounts.player.key();
    game.status = GameStatus::Active;
    // country_fee was set at creation time from live SOL/GBP rate — no recalculation needed.
    game.fees_advanced = game.fees_advanced.checked_add(CREATE_GAME_COST).ok_or(GameErrorCode::ArithmeticOverflow)?;
    game.updated_at = now;

    Ok(())
}
