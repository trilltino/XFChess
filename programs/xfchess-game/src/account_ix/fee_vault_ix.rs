//! Player session key and ELO update instructions.

use crate::errors::GameErrorCode;
use crate::state::PlayerSession;
use anchor_lang::prelude::*;

// ─── Player Session Instructions ──────────────────────────────────────────────

/// Creates a `PlayerSession` PDA: a time-bounded (24h default) session key
/// that can sign game transactions without a wallet popup. Distinct from the
/// longer-lived, multi-game session delegation in `global_session_ix.rs`.
#[derive(Accounts)]
#[instruction(session_key: Pubkey)]
pub struct CreateSession<'info> {
    #[account(mut)]
    pub player: Signer<'info>,
    #[account(
        init,
        payer = player,
        space = 8 + PlayerSession::INIT_SPACE,
        seeds = [PlayerSession::SEED, player.key().as_ref(), session_key.as_ref()],
        bump,
    )]
    pub session: Account<'info, PlayerSession>,
    pub system_program: Program<'info, System>,
}

/// Initializes the session with the given (or default) duration, spending
/// limit, and max-wager cap, and grants create/join/claim permissions.
pub fn handler_create_session(
    ctx: Context<CreateSession>,
    session_key: Pubkey,
    duration: Option<i64>,
    spending_limit: Option<u64>,
    max_wager: Option<u64>,
) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let s = &mut ctx.accounts.session;
    s.player = ctx.accounts.player.key();
    s.session_key = session_key;
    s.expires_at = now + duration.unwrap_or(PlayerSession::DEFAULT_DURATION);
    s.spending_limit = spending_limit.unwrap_or(PlayerSession::DEFAULT_SPENDING_LIMIT);
    s.max_wager = max_wager.unwrap_or(PlayerSession::MAX_WAGER_DEFAULT);
    s.total_spent = 0;
    s.games_played = 0;
    s.can_create_games = true;
    s.can_join_games = true;
    s.can_claim_prizes = true;
    s.is_active = true;
    s.bump = ctx.bumps.session;
    Ok(())
}

/// Closes a `PlayerSession` PDA and refunds its rent to the player.
#[derive(Accounts)]
pub struct RevokeSession<'info> {
    #[account(mut)]
    pub player: Signer<'info>,
    #[account(
        mut,
        close = player,
        has_one = player,
        seeds = [PlayerSession::SEED, player.key().as_ref(), session.session_key.as_ref()],
        bump = session.bump,
    )]
    pub session: Account<'info, PlayerSession>,
}

/// Marks the session inactive; the account then closes per the `close = player`
/// constraint on `RevokeSession`, returning rent to the player.
pub fn handler_revoke_session(ctx: Context<RevokeSession>) -> Result<()> {
    ctx.accounts.session.is_active = false;
    Ok(())
}

/// Updates a player's lifetime win/loss/draw/wager stats after a game.
/// Restricted to the VPS backend authority.
#[derive(Accounts)]
pub struct UpdateElo<'info> {
    /// VPS backend authority — only this key may update ELO standings.
    #[account(address = crate::constants::vps_authority::ID @ GameErrorCode::UnauthorizedAccess)]
    pub authority: Signer<'info>,
    #[account(mut)]
    pub profile: Account<'info, crate::state::PlayerProfile>,
}

/// Updates lifetime stats (wins/losses/draws/streaks/games_played) and, for
/// ranked games, wager totals. Does not touch `elo_rating` itself — that field
/// is updated exclusively by `finalize_game` (K=32). `_opponent_rating` and
/// `_opponent_rd` are unused here and kept only for ABI compatibility.
pub fn handler_update_elo(
    ctx: Context<UpdateElo>,
    _opponent_rating: u32,
    _opponent_rd: u32,
    outcome: u32, // 10000=win, 5000=draw, 0=loss
    is_ranked: bool,
    wager: u64,
    won_amount: u64,
) -> Result<()> {
    let p = &mut ctx.accounts.profile;
    // ELO rating (elo_rating field) is updated exclusively in finalize_game using K=32.
    // This instruction updates lifetime stats only (wins/losses/draws/wagered/won).
    // The _opponent_rating and _opponent_rd parameters are kept for ABI compatibility.

    match outcome {
        10000 => {
            p.wins = p
                .wins
                .checked_add(1)
                .ok_or(GameErrorCode::ArithmeticOverflow)?;
            p.win_streak = p
                .win_streak
                .checked_add(1)
                .ok_or(GameErrorCode::ArithmeticOverflow)?;
            if p.win_streak > p.best_streak {
                p.best_streak = p.win_streak;
            }
        }
        0 => {
            p.losses = p
                .losses
                .checked_add(1)
                .ok_or(GameErrorCode::ArithmeticOverflow)?;
            p.win_streak = 0;
        }
        _ => {
            p.draws = p
                .draws
                .checked_add(1)
                .ok_or(GameErrorCode::ArithmeticOverflow)?;
            p.win_streak = 0;
        }
    }
    p.games_played = p
        .games_played
        .checked_add(1)
        .ok_or(GameErrorCode::ArithmeticOverflow)?;
    p.last_game_at = Clock::get()?.unix_timestamp;

    if is_ranked {
        p.ranked_games = p
            .ranked_games
            .checked_add(1)
            .ok_or(GameErrorCode::ArithmeticOverflow)?;
        p.total_wagered = p
            .total_wagered
            .checked_add(wager)
            .ok_or(GameErrorCode::ArithmeticOverflow)?;
        p.total_won = p
            .total_won
            .checked_add(won_amount)
            .ok_or(GameErrorCode::ArithmeticOverflow)?;
    }
    Ok(())
}
