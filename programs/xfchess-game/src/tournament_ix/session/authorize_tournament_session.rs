use crate::constants::*;
use crate::errors::XfchessGameError;
use crate::state::{
    Tournament, TournamentPlayersShard, TournamentSessionDelegation, TournamentStatus,
};
use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug)]
pub struct AuthorizeTournamentSessionArgs {
    pub session_key: Pubkey,
    pub duration_secs: Option<i64>,
    pub spending_limit: Option<u64>,
    pub max_wager: Option<u64>,
    pub deposit_lamports: u64,
}

pub fn handler_authorize_tournament_session(
    ctx: Context<AuthorizeTournamentSessionCtx>,
    tournament_id: u64,
    args: AuthorizeTournamentSessionArgs,
) -> Result<()> {
    let tournament = &ctx.accounts.tournament;
    let delegation = &mut ctx.accounts.session_delegation;
    let player = &ctx.accounts.player;

    if args.deposit_lamports > 0 {
        anchor_lang::system_program::transfer(
            CpiContext::new(
                System::id(),
                anchor_lang::system_program::Transfer {
                    from: player.to_account_info(),
                    to: delegation.to_account_info(),
                },
            ),
            args.deposit_lamports,
        )?;
    }

    require_eq!(
        tournament.tournament_id,
        tournament_id,
        XfchessGameError::UnauthorizedAccess
    );

    require!(
        matches!(
            tournament.status,
            TournamentStatus::Registration | TournamentStatus::Active
        ),
        XfchessGameError::UnauthorizedAccess
    );

    let mut shards: Vec<&TournamentPlayersShard> = vec![&ctx.accounts.tournament_players_shard_0];
    if let Some(s) = ctx.accounts.tournament_players_shard_1.as_ref() {
        shards.push(s);
    }
    if let Some(s) = ctx.accounts.tournament_players_shard_2.as_ref() {
        shards.push(s);
    }
    if let Some(s) = ctx.accounts.tournament_players_shard_3.as_ref() {
        shards.push(s);
    }
    require!(
        crate::tournament_ix::shards::contains_player(&shards, *player.key),
        XfchessGameError::UnauthorizedAccess
    );

    let now = Clock::get()?.unix_timestamp;
    let duration = args
        .duration_secs
        .unwrap_or(TournamentSessionDelegation::DEFAULT_DURATION);
    require!(duration > 0, XfchessGameError::UnauthorizedAccess);
    require!(
        duration <= MAX_SESSION_DURATION_SECS,
        XfchessGameError::DurationTooLarge
    );
    let expires_at = now
        .checked_add(duration)
        .ok_or(XfchessGameError::ArithmeticOverflow)?;

    delegation.tournament_id = tournament_id;
    delegation.player = player.key();
    delegation.session_key = args.session_key;
    delegation.expires_at = expires_at;
    delegation.spending_limit = args
        .spending_limit
        .unwrap_or(TournamentSessionDelegation::DEFAULT_SPENDING_LIMIT);
    delegation.total_spent = 0;
    delegation.max_wager = args
        .max_wager
        .unwrap_or(TournamentSessionDelegation::DEFAULT_MAX_WAGER);
    delegation.games_played = 0;
    delegation.enabled = true;
    delegation.bump = ctx.bumps.session_delegation;

    Ok(())
}

pub fn handler_revoke_tournament_session(
    ctx: Context<RevokeTournamentSessionCtx>,
    _tournament_id: u64,
) -> Result<()> {
    let delegation = &mut ctx.accounts.session_delegation;
    let player = &ctx.accounts.player;

    require_keys_eq!(
        delegation.player,
        player.key(),
        XfchessGameError::UnauthorizedAccess
    );

    delegation.enabled = false;
    delegation.expires_at = Clock::get()?.unix_timestamp;
    Ok(())
}

#[derive(Accounts)]
#[instruction(tournament_id: u64)]
pub struct AuthorizeTournamentSessionCtx<'info> {
    #[account(
        seeds = [b"tournament", tournament_id.to_le_bytes().as_ref()],
        bump = tournament.bump,
    )]
    pub tournament: Account<'info, Tournament>,

    #[account(
        seeds = [TOURNAMENT_PLAYERS_SEED, &[0u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_0: Account<'info, TournamentPlayersShard>,
    #[account(
        seeds = [TOURNAMENT_PLAYERS_SEED, &[1u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_1: Option<Account<'info, TournamentPlayersShard>>,
    #[account(
        seeds = [TOURNAMENT_PLAYERS_SEED, &[2u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_2: Option<Account<'info, TournamentPlayersShard>>,
    #[account(
        seeds = [TOURNAMENT_PLAYERS_SEED, &[3u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_3: Option<Account<'info, TournamentPlayersShard>>,

    #[account(
        init,
        payer = player,
        seeds = [
            TournamentSessionDelegation::SEED,
            tournament_id.to_le_bytes().as_ref(),
            player.key().as_ref(),
        ],
        bump,
        space = 8 + TournamentSessionDelegation::INIT_SPACE,
    )]
    pub session_delegation: Account<'info, TournamentSessionDelegation>,

    #[account(mut)]
    pub player: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(tournament_id: u64)]
pub struct RevokeTournamentSessionCtx<'info> {
    #[account(
        mut,
        seeds = [
            TournamentSessionDelegation::SEED,
            tournament_id.to_le_bytes().as_ref(),
            player.key().as_ref(),
        ],
        bump = session_delegation.bump,
    )]
    pub session_delegation: Account<'info, TournamentSessionDelegation>,

    #[account(mut)]
    pub player: Signer<'info>,
}
