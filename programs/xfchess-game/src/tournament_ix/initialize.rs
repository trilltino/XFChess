use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(tournament_id: u64)]
pub struct InitializeTournament<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + Tournament::INIT_SPACE,
        seeds = [TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament: Account<'info, Tournament>,
    /// CHECK: PDA escrow holding entry fees until payout or refund.
    #[account(
        mut,
        seeds = [TOURNAMENT_ESCROW_SEED, &tournament_id.to_le_bytes()],
        bump
    )]
    pub escrow_pda: UncheckedAccount<'info>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<InitializeTournament>,
    tournament_id: u64,
    name: String,
    entry_fee: u64,
) -> Result<()> {
    require!(name.len() <= 64, GameErrorCode::InvalidGameStatus);

    let t = &mut ctx.accounts.tournament;
    t.tournament_id = tournament_id;
    t.authority = ctx.accounts.authority.key();
    t.name = name;
    t.entry_fee = entry_fee;
    t.prize_pool = 0;
    t.players = [Pubkey::default(); 4];
    t.player_elos = [0u32; 4];
    t.registered_count = 0;
    t.status = TournamentStatus::Registration;
    t.current_round = 0;
    t.semi_final_1 = Pubkey::default();
    t.semi_final_2 = Pubkey::default();
    t.final_match = Pubkey::default();
    t.winner = None;
    t.created_at = Clock::get()?.unix_timestamp;
    t.started_at = None;
    t.completed_at = None;
    t.bump = ctx.bumps.tournament;

    msg!(
        "Tournament {} '{}' created. Entry fee: {} lamports",
        tournament_id,
        t.name,
        entry_fee
    );
    Ok(())
}
