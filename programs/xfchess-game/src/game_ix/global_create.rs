//! Session-signed variant of `create_game` using a global persistent session key.
//!
//! The session key (hot key stored on VPS/client) co-signs game creation;
//! wager funds are drawn from the [`GlobalSessionDelegation`] vault.
//! The player wallet never has to sign — zero popup per game.

use crate::account_ix::session_guards;
use crate::constants::{GAME_SEED, MAX_WAGER_AMOUNT, MIN_WAGER_LAMPORTS, WAGER_ESCROW_SEED};
use crate::errors::GameErrorCode;
use crate::game_ix::common::{init_game_fields, InitGameArgs};
use crate::state::{Game, GlobalSessionDelegation, MatchType};
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(game_id: u64, wager_amount: u64, match_type: MatchType, platform_fee: u64, base_time_seconds: u64, increment_seconds: u16)]
pub struct GlobalCreateGame<'info> {
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
        init,
        payer = session_delegation,
        space = 8 + Game::INIT_SPACE,
        seeds = [GAME_SEED, &game_id.to_le_bytes()],
        bump
    )]
    pub game: Account<'info, Game>,

    /// CHECK: PDA for escrowing SOL wager.
    #[account(mut, seeds = [WAGER_ESCROW_SEED, &game_id.to_le_bytes()], bump)]
    pub escrow_pda: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<GlobalCreateGame>,
    game_id: u64,
    wager_amount: u64,
    match_type: MatchType,
    platform_fee: u64,
    base_time_seconds: u64,
    increment_seconds: u16,
) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let session = &ctx.accounts.session_delegation;

    require!(
        session.is_valid(now),
        GameErrorCode::SessionExpiredOrDisabled
    );
    require!(
        session.games_remaining > 0,
        GameErrorCode::GlobalSessionNoGamesRemaining
    );
    require!(
        wager_amount <= MAX_WAGER_AMOUNT,
        GameErrorCode::WagerTooHigh
    );
    require!(
        wager_amount == 0 || wager_amount >= MIN_WAGER_LAMPORTS,
        GameErrorCode::StakeTooLow
    );
    require!(
        session.has_budget(wager_amount),
        GameErrorCode::GlobalSessionSpendingLimitExceeded
    );

    // Transfer wager from delegation vault to escrow
    if wager_amount > 0 {
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
            wager_amount,
        )?;
    }

    // Update session bookkeeping
    let session = &mut ctx.accounts.session_delegation;
    session.total_spent = session_guards::checked_session_total(session.total_spent, wager_amount)?;
    session.games_remaining = session.games_remaining.saturating_sub(1);

    init_game_fields(
        &mut ctx.accounts.game,
        InitGameArgs {
            game_id,
            white: ctx.accounts.player.key(),
            fee_payer: ctx.accounts.session_signer.key(),
            wager_amount,
            match_type,
            platform_fee,
            base_time_seconds,
            increment_seconds,
            tournament_id: None,
        },
        now,
        ctx.bumps.game,
    )?;

    Ok(())
}
