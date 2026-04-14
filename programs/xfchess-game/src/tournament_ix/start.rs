//! Instruction to lock registration and seed players for bracket generation.
//! Match accounts are created separately via initialize_match instructions.

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
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

/// Sorts players by ELO descending and records seed order.
/// Backend uses this to generate matches via separate initialize_match calls.
pub fn handler(ctx: Context<StartTournament>, tournament_id: u64) -> Result<()> {
    let tournament = &mut ctx.accounts.tournament;

    require!(
        tournament.status == TournamentStatus::Registration,
        GameErrorCode::TournamentNotInRegistration
    );
    require!(
        tournament.registered_count == tournament.max_players,
        GameErrorCode::TournamentFull
    );

    let player_count = tournament.registered_count as usize;

    // Sort players by ELO descending
    let mut indexed: Vec<(usize, u32)> = tournament
        .player_elos
        .iter()
        .enumerate()
        .map(|(i, &elo)| (i, elo))
        .collect();
    indexed.sort_by(|a, b| b.1.cmp(&a.1));

    // Reorder players and elos by seed (highest ELO first)
    let mut seeded_players: Vec<Pubkey> = Vec::with_capacity(player_count);
    let mut seeded_elos: Vec<u32> = Vec::with_capacity(player_count);
    for (original_idx, elo) in indexed {
        seeded_players.push(tournament.players[original_idx]);
        seeded_elos.push(elo);
    }

    tournament.players = seeded_players;
    tournament.player_elos = seeded_elos;
    tournament.status = TournamentStatus::Active;
    tournament.current_round = 0;
    tournament.started_at = Some(Clock::get()?.unix_timestamp);

    msg!(
        "Tournament {} started with {} players. Players seeded by ELO.",
        tournament_id,
        player_count
    );

    // Log bracket matchups for first round
    let round1_matches = player_count / 2;
    for i in 0..round1_matches {
        let white_idx = i;
        let black_idx = player_count - 1 - i;
        msg!(
            "R1 Match {}: Seed {} ({}) vs Seed {} ({})",
            i,
            white_idx + 1,
            tournament.players[white_idx],
            black_idx + 1,
            tournament.players[black_idx]
        );
    }

    Ok(())
}
