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
        bump = tournament.bump,
        constraint = tournament.authority == authority.key() @ GameErrorCode::NotTournamentAuthority
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
    pub authority: Signer<'info>,
}

/// Finalises a Swiss tournament. Tiebreaks (Buchholz, Sonneborn-Berger) need each
/// player's full opponent history, which only the off-chain pairing engine holds, so
/// the authority supplies the final top places. The program checks that order
/// against on-chain scores: every listed player is registered and distinct, scores
/// never increase down the list, and nobody left off the list outscores its last entry.
pub fn handler(
    ctx: Context<CompleteSwissTournament>,
    tournament_id: u64,
    places: Vec<Pubkey>,
) -> Result<()> {
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
    require!(
        t.current_round >= t.total_rounds,
        GameErrorCode::SwissTournamentNotFinished
    );
    // Places are checked against every standing; an omitted shard would hide
    // players who outscore the submitted order.
    let present = [
        true,
        ctx.accounts.tournament_players_shard_1.is_some(),
        ctx.accounts.tournament_players_shard_2.is_some(),
        ctx.accounts.tournament_players_shard_3.is_some(),
    ];
    let required = crate::tournament_ix::shards::required_shards(t.max_players) as usize;
    require!(
        present[..required].iter().all(|shard| *shard),
        GameErrorCode::InvalidSwissPlaces
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

    validate_places(&standings, &places)?;

    let mut slots: [Option<Pubkey>; 10] = [None; 10];
    for (slot, player) in slots.iter_mut().zip(places.iter()) {
        *slot = Some(*player);
    }

    t.winner = slots[0];
    t.second_place = slots[1];
    t.third_place = slots[2];
    t.fourth_place = slots[3];
    t.fifth_place = slots[4];
    t.sixth_place = slots[5];
    t.seventh_place = slots[6];
    t.eighth_place = slots[7];
    t.ninth_place = slots[8];
    t.tenth_place = slots[9];

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

fn validate_places(standings: &[SwissStanding], places: &[Pubkey]) -> Result<()> {
    require!(
        places.len() == standings.len().min(10),
        GameErrorCode::InvalidSwissPlaces
    );

    let mut previous_score: Option<u8> = None;
    for (i, player) in places.iter().enumerate() {
        require!(
            !places[..i].contains(player),
            GameErrorCode::InvalidSwissPlaces
        );
        let score = standings
            .iter()
            .find(|s| s.player == *player)
            .ok_or(GameErrorCode::InvalidSwissPlaces)?
            .score;
        if let Some(prev) = previous_score {
            require!(score <= prev, GameErrorCode::InvalidSwissPlaces);
        }
        previous_score = Some(score);
    }

    if let Some(cutoff) = previous_score {
        require!(
            standings
                .iter()
                .filter(|s| !places.contains(&s.player))
                .all(|s| s.score <= cutoff),
            GameErrorCode::InvalidSwissPlaces
        );
    }
    Ok(())
}
