use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(tournament_id: u64)]
pub struct CompleteSwissTournament<'info> {
    #[account(
        mut,
        seeds = [TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        bump = tournament.bump
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
    pub cranker: Signer<'info>,
}

pub fn handler(ctx: Context<CompleteSwissTournament>, tournament_id: u64) -> Result<()> {
    let t = &mut ctx.accounts.tournament;

    require!(
        t.tournament_id == tournament_id,
        GameErrorCode::UnauthorizedAccess
    );
    require!(
        t.status == TournamentStatus::Active,
        GameErrorCode::InvalidGameStatus
    );
    require!(
        matches!(t.tournament_type, TournamentType::Swiss { .. }),
        GameErrorCode::InvalidGameStatus
    );
    // `advance_round` refuses to advance past `total_rounds`, so this can
    // only be true once every round has actually been played and cranked.
    require!(
        t.current_round >= t.total_rounds,
        GameErrorCode::SwissTournamentNotFinished
    );

    let mut standings: Vec<SwissStanding> = ctx
        .accounts
        .tournament_players_shard_0
        .swiss_standings
        .clone();
    for shard in [
        &ctx.accounts.tournament_players_shard_1,
        &ctx.accounts.tournament_players_shard_2,
        &ctx.accounts.tournament_players_shard_3,
    ]
    .into_iter()
    .flatten()
    {
        standings.extend(shard.swiss_standings.iter().cloned());
    }

    // Highest score first; Buchholz then Sonneborn-Berger break ties — the
    // standard Swiss tiebreak order, both already accumulated per-match by
    // `record_swiss_result`.
    standings.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then(b.buchholz.cmp(&a.buchholz))
            .then(b.sonneborn.cmp(&a.sonneborn))
    });

    let mut places: [Option<Pubkey>; 10] = [None; 10];
    for (i, standing) in standings.iter().take(10).enumerate() {
        places[i] = Some(standing.player);
    }

    t.winner = places[0];
    t.second_place = places[1];
    t.third_place = places[2];
    t.fourth_place = places[3];
    t.fifth_place = places[4];
    t.sixth_place = places[5];
    t.seventh_place = places[6];
    t.eighth_place = places[7];
    t.ninth_place = places[8];
    t.tenth_place = places[9];

    t.status = TournamentStatus::Completed;
    t.completed_at = Some(Clock::get()?.unix_timestamp);

    msg!(
        "Swiss tournament {} completed — {} players ranked, champion {:?}",
        tournament_id,
        standings.len(),
        t.winner
    );

    Ok(())
}
