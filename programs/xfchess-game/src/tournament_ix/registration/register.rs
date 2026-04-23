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
    /// CHECK: Host treasury wallet — receives entry fees directly.
    #[account(
        mut,
        constraint = host_treasury.key() == tournament.host_treasury @ GameErrorCode::UnauthorizedAccess
    )]
    pub host_treasury: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<RegisterPlayer>, _tournament_id: u64) -> Result<()> {
    let tournament = &mut ctx.accounts.tournament;
    let player_key = ctx.accounts.player.key();
    let player_elo = (ctx.accounts.player_profile.elo_rating / 100.0) as u32;

    require!(
        tournament.status == TournamentStatus::Registration,
        GameErrorCode::TournamentNotInRegistration
    );
    require!(
        tournament.registered_count < tournament.max_players,
        GameErrorCode::TournamentFull
    );

    // For USDC prize pool tournaments, require prize to be funded first
    if tournament.usdc_prize_mint.is_some() {
        require!(
            tournament.usdc_prize_funded,
            GameErrorCode::UsdcPrizeNotFunded
        );
    }

    // ELO filtering
    require!(
        player_elo >= tournament.elo_min,
        GameErrorCode::EloTooLow
    );
    require!(
        player_elo <= tournament.elo_max,
        GameErrorCode::EloTooHigh
    );

    // Check not already registered
    for existing in tournament.players.iter() {
        require!(*existing != player_key, GameErrorCode::AlreadyRegistered);
    }

    // Add player to vectors
    tournament.players.push(player_key);
    tournament.player_elos.push(player_elo);
    tournament.registered_count += 1;

    // Transfer entry fee directly to host treasury
    if tournament.entry_fee > 0 {
        anchor_lang::system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.player.to_account_info(),
                    to: ctx.accounts.host_treasury.to_account_info(),
                },
            ),
            tournament.entry_fee,
        )?;
    }

    msg!(
        "Player {} (ELO: {}) registered for tournament {} (slot {}/{}). Entry fee: {} lamports -> host treasury",
        player_key,
        player_elo,
        tournament.tournament_id,
        tournament.registered_count,
        tournament.max_players,
        tournament.entry_fee
    );
    Ok(())
}
