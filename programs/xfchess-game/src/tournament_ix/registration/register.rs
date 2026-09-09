use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use crate::tournament_ix::shards;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(tournament_id: u64, elo: u32)]
pub struct RegisterPlayer<'info> {
    #[account(
        mut,
        seeds = [TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        bump = tournament.bump
    )]
    pub tournament: Box<Account<'info, Tournament>>,
    #[account(
        seeds = [PROFILE_SEED, player.key().as_ref()],
        bump
    )]
    pub player_profile: Account<'info, PlayerProfile>,
    #[account(mut)]
    pub player: Signer<'info>,
    #[account(
        mut,
        seeds = [TOURNAMENT_ESCROW_SEED, &tournament_id.to_le_bytes()],
        bump
    )]
    pub escrow_pda: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds = [TOURNAMENT_PLAYERS_SEED, &[0u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_0: Box<Account<'info, TournamentPlayersShard>>,
    #[account(
        mut,
        seeds = [TOURNAMENT_PLAYERS_SEED, &[1u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_1: Option<Box<Account<'info, TournamentPlayersShard>>>,
    #[account(
        mut,
        seeds = [TOURNAMENT_PLAYERS_SEED, &[2u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_2: Option<Box<Account<'info, TournamentPlayersShard>>>,
    #[account(
        mut,
        seeds = [TOURNAMENT_PLAYERS_SEED, &[3u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_3: Option<Box<Account<'info, TournamentPlayersShard>>>,
    #[account(
        mut,
        constraint = host_treasury.key() == tournament.host_treasury @ GameErrorCode::UnauthorizedAccess
    )]
    pub host_treasury: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<RegisterPlayer>, tournament_id: u64, elo: u32) -> Result<()> {
    let tournament = &mut ctx.accounts.tournament;
    require!(
        tournament.tournament_id == tournament_id,
        GameErrorCode::UnauthorizedAccess
    );
    let player = ctx.accounts.player.key();

    require!(
        tournament.status == TournamentStatus::Registration,
        GameErrorCode::InvalidTournamentStatus
    );
    require!(
        tournament.num_registered_players < tournament.max_players,
        GameErrorCode::TournamentFull
    );
    // Paid tournaments must have a guaranteed prize locked in escrow before any
    // player can register — SOL (prize_pool) or USDC (usdc_prize_funded).
    if tournament.entry_fee > 0 {
        require!(
            tournament.prize_pool > 0 || tournament.usdc_prize_funded,
            GameErrorCode::PrizeNotFunded
        );
    }
    require!(
        tournament.elo_min <= elo && elo <= tournament.elo_max,
        GameErrorCode::EloOutOfRange
    );

    // Check for duplicate registration across all present shards.
    // Shards that weren't initialized (None) are skipped — they hold no players.
    let mut present_shards: Vec<&TournamentPlayersShard> =
        vec![&ctx.accounts.tournament_players_shard_0];
    if let Some(s) = ctx.accounts.tournament_players_shard_1.as_ref() {
        present_shards.push(s);
    }
    if let Some(s) = ctx.accounts.tournament_players_shard_2.as_ref() {
        present_shards.push(s);
    }
    if let Some(s) = ctx.accounts.tournament_players_shard_3.as_ref() {
        present_shards.push(s);
    }
    require!(
        !shards::contains_player(&present_shards, player),
        GameErrorCode::AlreadyRegistered
    );

    // Determine which shard this player slots into.
    let shard_id =
        (tournament.num_registered_players / TournamentPlayersShard::SHARD_CAPACITY) as u8;

    match shard_id {
        0 => {
            let s = &mut ctx.accounts.tournament_players_shard_0;
            shards::push_player(s, player, elo)?;
        }
        1 => {
            let s = ctx
                .accounts
                .tournament_players_shard_1
                .as_mut()
                .ok_or(GameErrorCode::TournamentFull)?;
            shards::push_player(s, player, elo)?;
        }
        2 => {
            let s = ctx
                .accounts
                .tournament_players_shard_2
                .as_mut()
                .ok_or(GameErrorCode::TournamentFull)?;
            shards::push_player(s, player, elo)?;
        }
        3 => {
            let s = ctx
                .accounts
                .tournament_players_shard_3
                .as_mut()
                .ok_or(GameErrorCode::TournamentFull)?;
            shards::push_player(s, player, elo)?;
        }
        _ => return Err(GameErrorCode::TournamentFull.into()),
    }

    tournament.num_registered_players = tournament
        .num_registered_players
        .checked_add(1)
        .ok_or(GameErrorCode::ArithmeticOverflow)?;
    tournament.player_count = tournament
        .player_count
        .checked_add(1)
        .ok_or(GameErrorCode::ArithmeticOverflow)?;

    // Entry fee → escrow PDA as a refundable deposit. It never touches the prize
    // pool (which the operator locked before registration opened) and is only
    // swept to host_treasury when the tournament actually starts.
    if tournament.entry_fee > 0 {
        anchor_lang::system_program::transfer(
            CpiContext::new(
                System::id(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.player.to_account_info(),
                    to: ctx.accounts.escrow_pda.to_account_info(),
                },
            ),
            tournament.entry_fee,
        )?;
    }

    msg!(
        "Player {} registered with ELO {} in shard {}",
        player,
        elo,
        shard_id
    );
    Ok(())
}
