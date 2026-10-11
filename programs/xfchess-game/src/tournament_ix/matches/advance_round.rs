use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;

/// Closes the current Swiss round. Withdrawals mean a round can have fewer board
/// slots (pairings plus the bye, if any) than the field allows, so the authority
/// states how many the round used and every one of them must be reported.
#[derive(Accounts)]
#[instruction(tournament_id: u64)]
pub struct AdvanceRound<'info> {
    #[account(
        mut,
        seeds = [TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        bump = tournament.bump,
        constraint = tournament.authority == authority.key() @ GameErrorCode::NotTournamentAuthority
    )]
    pub tournament: Account<'info, Tournament>,
    pub authority: Signer<'info>,
}

pub fn handler(ctx: Context<AdvanceRound>, tournament_id: u64, boards_in_round: u16) -> Result<()> {
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
        t.current_round < t.total_rounds,
        GameErrorCode::InvalidGameStatus
    );

    let boards_per_round = super::record_swiss_result::board_slots(t.num_registered_players);
    require!(
        boards_in_round <= boards_per_round,
        GameErrorCode::InvalidArgument
    );
    require!(
        super::round_bitmap::all_set(&t.round_boards_reported, boards_in_round),
        GameErrorCode::TournamentRoundIncomplete
    );

    t.current_round += 1;
    t.round_boards_reported = [0u8; 16];

    msg!(
        "Tournament {} advanced to round {} of {} ({} boards closed)",
        tournament_id,
        t.current_round,
        t.total_rounds,
        boards_in_round
    );

    Ok(())
}
