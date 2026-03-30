use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
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
    #[account(
        init,
        payer = authority,
        space = 8 + TournamentMatch::INIT_SPACE,
        seeds = [TOURNAMENT_MATCH_SEED, &tournament_id.to_le_bytes(), &[0u8]],
        bump
    )]
    pub semi_final_1: Account<'info, TournamentMatch>,
    #[account(
        init,
        payer = authority,
        space = 8 + TournamentMatch::INIT_SPACE,
        seeds = [TOURNAMENT_MATCH_SEED, &tournament_id.to_le_bytes(), &[1u8]],
        bump
    )]
    pub semi_final_2: Account<'info, TournamentMatch>,
    #[account(
        init,
        payer = authority,
        space = 8 + TournamentMatch::INIT_SPACE,
        seeds = [TOURNAMENT_MATCH_SEED, &tournament_id.to_le_bytes(), &[2u8]],
        bump
    )]
    pub final_match: Account<'info, TournamentMatch>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<StartTournament>, tournament_id: u64) -> Result<()> {
    require!(
        ctx.accounts.tournament.status == TournamentStatus::Registration,
        GameErrorCode::TournamentNotInRegistration
    );
    require!(
        ctx.accounts.tournament.registered_count == 4,
        GameErrorCode::TournamentFull
    );

    // Seed players by ELO descending: highest vs lowest, second vs third
    let players = ctx.accounts.tournament.players;
    let elos = ctx.accounts.tournament.player_elos;
    let mut indexed: [(usize, u32); 4] = [(0, elos[0]), (1, elos[1]), (2, elos[2]), (3, elos[3])];
    indexed.sort_by(|a, b| b.1.cmp(&a.1));

    // SF1: seed[0] (white) vs seed[3] (black)
    let sf1_white = players[indexed[0].0];
    let sf1_black = players[indexed[3].0];
    // SF2: seed[1] (white) vs seed[2] (black)
    let sf2_white = players[indexed[1].0];
    let sf2_black = players[indexed[2].0];

    let sf1_key = ctx.accounts.semi_final_1.key();
    let sf2_key = ctx.accounts.semi_final_2.key();
    let final_key = ctx.accounts.final_match.key();
    let now = Clock::get()?.unix_timestamp;
    let sf1_bump = ctx.bumps.semi_final_1;
    let sf2_bump = ctx.bumps.semi_final_2;
    let fin_bump = ctx.bumps.final_match;

    ctx.accounts.semi_final_1.tournament_id = tournament_id;
    ctx.accounts.semi_final_1.match_index = 0;
    ctx.accounts.semi_final_1.round = 0;
    ctx.accounts.semi_final_1.player_white = Some(sf1_white);
    ctx.accounts.semi_final_1.player_black = Some(sf1_black);
    ctx.accounts.semi_final_1.winner = None;
    ctx.accounts.semi_final_1.game_pda = None;
    ctx.accounts.semi_final_1.game_id = None;
    ctx.accounts.semi_final_1.status = MatchStatus::Pending;
    ctx.accounts.semi_final_1.started_at = None;
    ctx.accounts.semi_final_1.completed_at = None;
    ctx.accounts.semi_final_1.bump = sf1_bump;

    ctx.accounts.semi_final_2.tournament_id = tournament_id;
    ctx.accounts.semi_final_2.match_index = 1;
    ctx.accounts.semi_final_2.round = 0;
    ctx.accounts.semi_final_2.player_white = Some(sf2_white);
    ctx.accounts.semi_final_2.player_black = Some(sf2_black);
    ctx.accounts.semi_final_2.winner = None;
    ctx.accounts.semi_final_2.game_pda = None;
    ctx.accounts.semi_final_2.game_id = None;
    ctx.accounts.semi_final_2.status = MatchStatus::Pending;
    ctx.accounts.semi_final_2.started_at = None;
    ctx.accounts.semi_final_2.completed_at = None;
    ctx.accounts.semi_final_2.bump = sf2_bump;

    ctx.accounts.final_match.tournament_id = tournament_id;
    ctx.accounts.final_match.match_index = 2;
    ctx.accounts.final_match.round = 1;
    ctx.accounts.final_match.player_white = None;
    ctx.accounts.final_match.player_black = None;
    ctx.accounts.final_match.winner = None;
    ctx.accounts.final_match.game_pda = None;
    ctx.accounts.final_match.game_id = None;
    ctx.accounts.final_match.status = MatchStatus::Pending;
    ctx.accounts.final_match.started_at = None;
    ctx.accounts.final_match.completed_at = None;
    ctx.accounts.final_match.bump = fin_bump;

    ctx.accounts.tournament.semi_final_1 = sf1_key;
    ctx.accounts.tournament.semi_final_2 = sf2_key;
    ctx.accounts.tournament.final_match = final_key;
    ctx.accounts.tournament.status = TournamentStatus::Active;
    ctx.accounts.tournament.current_round = 0;
    ctx.accounts.tournament.started_at = Some(now);

    msg!(
        "Tournament {} started. SF1: {} vs {} | SF2: {} vs {}",
        tournament_id,
        sf1_white,
        sf1_black,
        sf2_white,
        sf2_black,
    );
    Ok(())
}
