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
    /// CHECK: PDA escrow that holds entry fees.
    #[account(
        mut,
        seeds = [TOURNAMENT_ESCROW_SEED, &tournament_id.to_le_bytes()],
        bump
    )]
    pub escrow_pda: UncheckedAccount<'info>,
    #[account(
        seeds = [PROFILE_SEED, player.key().as_ref()],
        bump
    )]
    pub player_profile: Account<'info, PlayerProfile>,
    #[account(mut)]
    pub player: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<RegisterPlayer>, _tournament_id: u64) -> Result<()> {
    let tournament = &mut ctx.accounts.tournament;
    let player_key = ctx.accounts.player.key();

    require!(
        tournament.status == TournamentStatus::Registration,
        GameErrorCode::TournamentNotInRegistration
    );
    require!(
        tournament.registered_count < 4,
        GameErrorCode::TournamentFull
    );

    // Check not already registered
    for existing in tournament.players.iter() {
        require!(*existing != player_key, GameErrorCode::AlreadyRegistered);
    }

    let slot = tournament.registered_count as usize;
    tournament.players[slot] = player_key;
    tournament.player_elos[slot] = ctx.accounts.player_profile.elo as u32;
    tournament.registered_count += 1;

    // Transfer entry fee to escrow
    if tournament.entry_fee > 0 {
        anchor_lang::system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.player.to_account_info(),
                    to: ctx.accounts.escrow_pda.to_account_info(),
                },
            ),
            tournament.entry_fee,
        )?;
        tournament.prize_pool += tournament.entry_fee;
    }

    msg!(
        "Player {} registered for tournament {} (slot {}/4)",
        player_key,
        tournament.tournament_id,
        tournament.registered_count
    );
    Ok(())
}
