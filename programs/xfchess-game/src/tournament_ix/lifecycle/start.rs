//! Instruction to lock registration and seed players for bracket generation.
//! Match accounts are created separately via initialize_match instructions.

use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use crate::tournament_ix::shards;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(tournament_id: u64)]
pub struct StartTournament<'info> {
    #[account(
        mut,
        seeds = [TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        bump = tournament.bump,
        constraint = tournament.authority == authority.key() @ GameErrorCode::NotTournamentAuthority
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
    /// CHECK: Tournament escrow PDA — entry-fee deposits are swept from here to
    /// host_treasury once the tournament actually starts.
    #[account(
        mut,
        seeds = [TOURNAMENT_ESCROW_SEED, &tournament_id.to_le_bytes()],
        bump
    )]
    pub escrow_pda: UncheckedAccount<'info>,
    /// CHECK: Operator treasury — receives the swept entry fees (operator revenue).
    #[account(
        mut,
        constraint = host_treasury.key() == tournament.host_treasury @ GameErrorCode::UnauthorizedAccess
    )]
    pub host_treasury: UncheckedAccount<'info>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

/// Sorts players by ELO descending and records seed order.
/// Backend uses this to generate matches via separate initialize_match calls.
pub fn handler(ctx: Context<StartTournament>, tournament_id: u64) -> Result<()> {
    let tournament = &mut ctx.accounts.tournament;
    require!(
        tournament.tournament_id == tournament_id,
        GameErrorCode::UnauthorizedAccess
    );
    require!(
        tournament.status == TournamentStatus::Registration,
        GameErrorCode::TournamentNotInRegistration
    );
    // Swiss tournaments may start at min_players (pairing handles any count);
    // single-elimination needs the full bracket. Below min_players the backend
    // should call cancel_tournament instead, which refunds every entry fee.
    match tournament.tournament_type {
        TournamentType::Swiss { .. } => {
            require!(
                tournament.num_registered_players >= tournament.min_players,
                GameErrorCode::MinPlayersNotReached
            );
        }
        TournamentType::SingleElimination => {
            require!(
                tournament.num_registered_players == tournament.max_players,
                GameErrorCode::TournamentFull
            );
        }
    }

    let player_count = tournament.num_registered_players as usize;

    // Small/medium tournaments only initialize shard 0 (or 0-1); the missing
    // shard accounts are passed as the program ID and resolve to None.
    let required = shards::required_shards(tournament.max_players) as usize;

    {
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
        require!(
            shard_refs.len() >= required,
            GameErrorCode::InvalidTournamentStatus
        );

        let collected = shards::collect_players(&shard_refs)?;
        require!(
            collected.len() == player_count,
            GameErrorCode::InvalidTournamentStatus
        );

        // Sort players by ELO descending
        let mut seeded = collected;
        seeded.sort_by(|a, b| b.1.cmp(&a.1));

        let mut shard_muts: Vec<&mut TournamentPlayersShard> =
            vec![&mut ctx.accounts.tournament_players_shard_0];
        if let Some(s) = ctx.accounts.tournament_players_shard_1.as_mut() {
            shard_muts.push(s);
        }
        if let Some(s) = ctx.accounts.tournament_players_shard_2.as_mut() {
            shard_muts.push(s);
        }
        if let Some(s) = ctx.accounts.tournament_players_shard_3.as_mut() {
            shard_muts.push(s);
        }

        for shard in shard_muts.iter_mut() {
            shard.players.clear();
            shard.player_elos.clear();
            shard.swiss_standings.clear();
        }

        for (i, (player, elo)) in seeded.iter().copied().enumerate() {
            let shard_id = i / TournamentPlayersShard::SHARD_CAPACITY as usize;
            let shard = shard_muts
                .get_mut(shard_id)
                .ok_or(GameErrorCode::TournamentFull)?;
            shards::push_player(shard, player, elo)?;
        }

        if matches!(tournament.tournament_type, TournamentType::Swiss { .. }) {
            shards::initialize_swiss_standings(&mut shard_muts)?;
        }
    }

    // The tournament is definitely running: sweep the entry-fee deposits from
    // escrow to the operator treasury. What remains in escrow afterwards is
    // exactly the guaranteed SOL prize (prize_pool) locked before registration.
    let fees_collected = tournament
        .entry_fee
        .checked_mul(tournament.num_registered_players as u64)
        .ok_or(GameErrorCode::ArithmeticOverflow)?;
    if fees_collected > 0 {
        require!(
            ctx.accounts.escrow_pda.lamports() >= fees_collected,
            GameErrorCode::InsufficientFunds
        );
        **ctx.accounts.escrow_pda.lamports.borrow_mut() -= fees_collected;
        **ctx.accounts.host_treasury.lamports.borrow_mut() += fees_collected;
        tournament.platform_fee_pool = fees_collected;
    }

    tournament.status = TournamentStatus::Active;
    tournament.current_round = 0;
    tournament.started_at = Some(Clock::get()?.unix_timestamp);

    Ok(())
}
