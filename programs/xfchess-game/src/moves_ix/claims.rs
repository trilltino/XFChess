use anchor_lang::prelude::*;
use crate::{constants::GAME_SEED, errors::GameErrorCode as E, state::{Game, SessionDelegation, GlobalSessionDelegation}};

#[derive(Accounts)]
#[instruction(game_id: u64)]
pub struct ClaimDraw<'info> {
    #[account(mut, seeds = [GAME_SEED, &game_id.to_le_bytes()], bump = game.bump)]
    pub game: Account<'info, Game>,
    pub claimant: Signer<'info>,
    pub session_delegation: Option<Account<'info, SessionDelegation>>,
    pub global_session: Option<Account<'info, GlobalSessionDelegation>>,
}

pub fn authenticated_player(ctx: &ClaimDraw<'_>, now: i64) -> Result<Pubkey> {
    let signer = ctx.claimant.key();
    if signer == ctx.game.white || signer == ctx.game.black {
        require!(ctx.session_delegation.is_none() && ctx.global_session.is_none(), E::InvalidSession);
        return Ok(signer);
    }
    let player = match (&ctx.session_delegation, &ctx.global_session) {
        (Some(session), None) => {
            let (expected, bump) = Pubkey::find_program_address(&[b"session_delegation", &ctx.game.game_id.to_le_bytes(), session.player.as_ref()], &crate::ID);
            require!(session.key() == expected && session.bump == bump && session.game_id == ctx.game.game_id, E::InvalidSession);
            require!(session.enabled && now <= session.expires_at && session.session_key == signer, E::SessionExpiredOrDisabled);
            session.player
        }
        (None, Some(session)) => {
            let (expected, bump) = Pubkey::find_program_address(&[GlobalSessionDelegation::SEED, session.player.as_ref()], &crate::ID);
            require!(session.key() == expected && session.bump == bump, E::InvalidSession);
            // Exhausting new-game budget does not revoke rights in an active game.
            require!(session.enabled && now <= session.expires_at && session.session_key == signer, E::SessionExpiredOrDisabled);
            session.player
        }
        _ => return err!(E::InvalidSessionKey),
    };
    require!(player == ctx.game.white || player == ctx.game.black, E::NotInGame);
    Ok(player)
}

pub fn handler(ctx: Context<ClaimDraw>, _game_id: u64, expected_nonce: u64, claim_nonce: u64, intended_move: Option<[u8; 5]>, repetition: bool) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let player = authenticated_player(ctx.accounts, now)?;
    #[cfg(feature = "move-validation")]
    {
        let info = ctx.accounts.game.to_account_info();
        crate::fide::adjudicate_claim(&mut ctx.accounts.game, &info, player, expected_nonce, claim_nonce, intended_move, repetition, now)
    }
    #[cfg(not(feature = "move-validation"))]
    { let _ = (player, expected_nonce, claim_nonce, intended_move, repetition); err!(E::LegacyGameRules) }
}
