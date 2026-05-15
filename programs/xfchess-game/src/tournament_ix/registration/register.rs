//! Instruction allowing players to opt-in and pay their entry fee for the tournament.
//! Entry fee goes directly to host_treasury (operator's wallet), not into escrow.

use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(tournament_id: u64)]
pub struct RegisterPlayer<'info> {
    #[account(
        mut,
        seeds = [TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        bump = tournament.bump
    )]
    pub tournament: Account<'info, Tournament>,
    #[account(
        seeds = [PROFILE_SEED, player.key().as_ref()],
        bump
    )]
    pub player_profile: Account<'info, PlayerProfile>,
    #[account(mut)]
    pub player: Signer<'info>,
    /// CHECK: Tournament escrow PDA — holds entry fees (prize pool).
    #[account(
        mut,
        seeds = [TOURNAMENT_ESCROW_SEED, &tournament_id.to_le_bytes()],
        bump
    )]
    pub escrow_pda: UncheckedAccount<'info>,
    /// The platform treasury vault (receives platform fees).
    #[account(
        mut,
        constraint = treasury_vault.key() == platform_treasury_vault.key() @ GameErrorCode::UnauthorizedAccess
    )]
    pub treasury_vault: Signer<'info>,
    /// CHECK: The platform treasury vault — must match the hardcoded pubkey.
    pub platform_treasury_vault: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<RegisterPlayer>, tournament_id: u64, elo: u32) -> Result<()> {
    let tournament = &mut ctx.accounts.tournament;
    let player = ctx.accounts.player.key();
    let _platform_treasury_vault = ctx.accounts.platform_treasury_vault.key();

    // Validate tournament state
    require!(
        tournament.status == TournamentStatus::Registration,
        GameErrorCode::InvalidTournamentStatus
    );
    require!(
        tournament.num_registered_players < tournament.max_players,
        GameErrorCode::TournamentFull
    );
    require!(
        tournament.elo_min <= elo && elo <= tournament.elo_max,
        GameErrorCode::EloOutOfRange
    );

    // Check for duplicate registration
    for i in 0..tournament.num_registered_players as usize {
        require!(
            tournament.players[i] != player,
            GameErrorCode::AlreadyRegistered
        );
    }

    // Record player
    let index = tournament.num_registered_players as usize;
    tournament.players[index] = player;
    tournament.player_elos[index] = elo;
    tournament.num_registered_players += 1;

    // Transfer entry fee + platform fee
    let entry_fee_total = tournament.entry_fee + tournament.platform_fee;
    let player_lamports = ctx.accounts.player.lamports();
    require!(
        player_lamports >= entry_fee_total,
        GameErrorCode::InsufficientFunds
    );

    **ctx.accounts.player.lamports.borrow_mut() -= entry_fee_total;
    **ctx.accounts.escrow_pda.lamports.borrow_mut() += tournament.entry_fee;
    **ctx.accounts.platform_treasury_vault.lamports.borrow_mut() += tournament.platform_fee;

    // Update prize pool
    tournament.prize_pool += tournament.entry_fee;

    Ok(())
}
