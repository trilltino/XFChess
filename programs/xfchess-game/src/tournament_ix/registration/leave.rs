use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use crate::tournament_ix::lifecycle::initialize_escrow::TournamentEscrow;
use crate::tournament_ix::shards;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(tournament_id: u64)]
pub struct LeaveTournament<'info> {
    #[account(
        mut,
        seeds = [TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        bump = tournament.bump
    )]
    pub tournament: Account<'info, Tournament>,
    #[account(
        mut,
        seeds = [TOURNAMENT_PLAYERS_SEED, &[0u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_0: Account<'info, TournamentPlayersShard>,
    #[account(
        mut,
        seeds = [TOURNAMENT_PLAYERS_SEED, &[1u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_1: Option<Account<'info, TournamentPlayersShard>>,
    #[account(
        mut,
        seeds = [TOURNAMENT_PLAYERS_SEED, &[2u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_2: Option<Account<'info, TournamentPlayersShard>>,
    #[account(
        mut,
        seeds = [TOURNAMENT_PLAYERS_SEED, &[3u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_3: Option<Account<'info, TournamentPlayersShard>>,
    #[account(mut)]
    pub player: Signer<'info>,
    #[account(
        mut,
        seeds = [TOURNAMENT_ESCROW_SEED, &tournament_id.to_le_bytes()],
        bump
    )]
    pub escrow_pda: Account<'info, TournamentEscrow>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<LeaveTournament>, tournament_id: u64) -> Result<()> {
    let tournament = &mut ctx.accounts.tournament;
    let player_key = ctx.accounts.player.key();

    require!(
        tournament.tournament_id == tournament_id,
        GameErrorCode::UnauthorizedAccess
    );

    require!(
        tournament.status == TournamentStatus::Registration,
        GameErrorCode::InvalidTournamentStatus
    );

    let mut shard_refs: Vec<&TournamentPlayersShard> =
        vec![&ctx.accounts.tournament_players_shard_0];
    if let Some(s) = ctx.accounts.tournament_players_shard_1.as_ref() {
        shard_refs.push(s);
    }
    if let Some(s) = ctx.accounts.tournament_players_shard_2.as_ref() {
        shard_refs.push(s);
    }
    if let Some(s) = ctx.accounts.tournament_players_shard_3.as_ref() {
        shard_refs.push(s);
    }
    let (shard_id, index) =
        shards::find_player(&shard_refs, player_key).ok_or(GameErrorCode::PlayerNotFound)?;

    let target_shard: &mut TournamentPlayersShard = match shard_id {
        0 => &mut ctx.accounts.tournament_players_shard_0,
        1 => ctx
            .accounts
            .tournament_players_shard_1
            .as_mut()
            .ok_or(GameErrorCode::PlayerNotFound)?,
        2 => ctx
            .accounts
            .tournament_players_shard_2
            .as_mut()
            .ok_or(GameErrorCode::PlayerNotFound)?,
        3 => ctx
            .accounts
            .tournament_players_shard_3
            .as_mut()
            .ok_or(GameErrorCode::PlayerNotFound)?,
        _ => return Err(GameErrorCode::PlayerNotFound.into()),
    };

    shards::remove_player(target_shard, index)?;

    tournament.num_registered_players = tournament
        .num_registered_players
        .checked_sub(1)
        .ok_or(GameErrorCode::ArithmeticOverflow)?;
    tournament.player_count = tournament
        .player_count
        .checked_sub(1)
        .ok_or(GameErrorCode::ArithmeticOverflow)?;

    let refund_amount = tournament.entry_fee;
    if refund_amount > 0 {
        crate::common::escrow::debit_program_pda(
            &ctx.accounts.escrow_pda.to_account_info(),
            &ctx.accounts.player.to_account_info(),
            refund_amount,
        )
        .map_err(|_| GameErrorCode::InsufficientTreasuryForRefund)?;
    }

    Ok(())
}
