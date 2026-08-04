//! Instruction allowing players to voluntarily leave a tournament before it starts and receive a refund.
//! The entry fee is refunded from the tournament escrow PDA — the operator's wallet is not involved.

use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
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
    /// TournamentPlayersShard 0 always present (all tournament sizes)
    #[account(
        mut,
        seeds = [TOURNAMENT_PLAYERS_SEED, &[0u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_0: Account<'info, TournamentPlayersShard>,
    /// TournamentPlayersShard 1 — present for >64-player tournaments only.
    /// Pass the program ID in its place for smaller tournaments.
    #[account(
        mut,
        seeds = [TOURNAMENT_PLAYERS_SEED, &[1u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_1: Option<Account<'info, TournamentPlayersShard>>,
    /// TournamentPlayersShard 2 — present for 256-player tournaments only.
    #[account(
        mut,
        seeds = [TOURNAMENT_PLAYERS_SEED, &[2u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_2: Option<Account<'info, TournamentPlayersShard>>,
    /// TournamentPlayersShard 3 — present for 256-player tournaments only.
    #[account(
        mut,
        seeds = [TOURNAMENT_PLAYERS_SEED, &[3u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_3: Option<Account<'info, TournamentPlayersShard>>,
    #[account(mut)]
    pub player: Signer<'info>,
    /// CHECK: Tournament escrow PDA — entry fees are held here, not in the operator's wallet.
    #[account(
        mut,
        seeds = [TOURNAMENT_ESCROW_SEED, &tournament_id.to_le_bytes()],
        bump
    )]
    pub escrow_pda: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<LeaveTournament>, tournament_id: u64) -> Result<()> {
    let tournament = &mut ctx.accounts.tournament;
    require!(
        tournament.tournament_id == tournament_id,
        GameErrorCode::UnauthorizedAccess
    );
    let player_key = ctx.accounts.player.key();

    // Validate tournament state
    require!(
        tournament.status == TournamentStatus::Registration,
        GameErrorCode::InvalidTournamentStatus
    );

    // Shards 1-3 are optional — small/medium tournaments only initialize shard 0
    // (or 0-1); missing shards are passed as the program ID and resolve to None.
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

    // Get mutable reference to the correct shard
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

    // Refund the entry-fee deposit from the tournament escrow PDA. The guaranteed
    // prize (tournament.prize_pool) is untouched — it was operator-funded before
    // registration and does not change with entry count.
    let refund_amount = tournament.entry_fee;
    if refund_amount > 0 {
        require!(
            ctx.accounts.escrow_pda.lamports() >= refund_amount,
            GameErrorCode::InsufficientTreasuryForRefund
        );
        **ctx.accounts.escrow_pda.lamports.borrow_mut() -= refund_amount;
        **ctx.accounts.player.lamports.borrow_mut() += refund_amount;
    }

    Ok(())
}
