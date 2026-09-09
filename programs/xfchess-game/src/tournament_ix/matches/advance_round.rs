use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(tournament_id: u64)]
pub struct AdvanceRound<'info> {
    #[account(
        mut,
        seeds = [TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        bump = tournament.bump
    )]
    pub tournament: Account<'info, Tournament>,
    pub cranker: Signer<'info>,
}

pub fn handler(ctx: Context<AdvanceRound>, tournament_id: u64) -> Result<()> {
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
    // Nothing to advance to once the last round has been played — final
    // standings/winner determination (a separate, not-yet-on-chain path)
    // takes over from here.
    require!(
        t.current_round < t.total_rounds,
        GameErrorCode::InvalidGameStatus
    );

    let boards_per_round = t.num_registered_players.max(2) / 2;
    require!(
        super::round_bitmap::all_set(&t.round_boards_reported, boards_per_round),
        GameErrorCode::TournamentRoundIncomplete
    );

    t.current_round += 1;
    t.round_boards_reported = [0u8; 16];

    msg!(
        "Tournament {} advanced to round {} of {}",
        tournament_id,
        t.current_round,
        t.total_rounds
    );

    Ok(())
}
