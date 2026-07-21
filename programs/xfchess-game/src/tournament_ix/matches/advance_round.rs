//! Permissionless instruction to advance a Swiss tournament to its next
//! round once every board in the current round has reported a result.
//!
//! Round advancement was previously inferred only by the backend scheduler
//! (see the removed comment in `record_swiss_result`), which meant a
//! tournament could stall forever mid-round if the backend process was gone
//! for good — even though registration, entry fees, and prizes are already
//! on-chain. This closes that gap: any caller (the backend, a recovery CLI,
//! or a player) can crank the round forward, and safety comes from
//! `Tournament::round_boards_reported` (set by `record_swiss_result`) proving
//! every board actually reported, not from who the caller is. Mirrors the
//! "anyone may crank" pattern already established by
//! `tournament_ix::prizes::distribute::DistributeTournamentPrizes`.
//!
//! Deliberately does NOT compute or store the next round's pairing — Swiss
//! pairing needs opponent/color history this program doesn't keep on-chain
//! (see `crates/swiss-pairing`), so any client can compute it locally from
//! the on-chain standings and communicate it to the matched players directly
//! (the same way `DirectConnection` already lets two known peers connect
//! without the backend). This instruction's only job is to be the durable,
//! backend-independent gate on "is this round actually over."
//!
//! Also deliberately does NOT compute final standings/winners when the last
//! round completes — that's a separate, pre-existing gap (Swiss tournaments
//! have no on-chain winner-determination path at all yet, unlike
//! single-elimination's `record_result::handler`) and out of scope here.

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
    /// Anyone may crank; safety comes from `round_boards_reported` already
    /// proving on-chain that every board is in, not from caller identity.
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
