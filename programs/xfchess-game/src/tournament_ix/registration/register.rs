//! Instruction allowing players to opt-in and pay their entry fee for the tournament.
//! Entry fee is transferred directly into the tournament escrow PDA — the operator
//! cannot access it until match logic executes and distributes funds to winners.
//!
//! Shards 1-3 are optional — small/medium tournaments only initialize shard 0 or 0-1.
//! Pass the remaining shards as None / the zero pubkey in those cases.

use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(tournament_id: u64, elo: u32)]
pub struct RegisterPlayer<'info> {
    #[account(
        mut,
        seeds = [TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        bump = tournament.bump
    )]
    pub tournament: Box<Account<'info, Tournament>>,
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
    /// Shard 0 always present (all tournament sizes).
    #[account(
        mut,
        seeds = [TOURNAMENT_PLAYERS_SEED, &[0u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_0: Box<Account<'info, TournamentPlayersShard>>,
    /// Shard 1 — present for ≥128-player tournaments only.
    #[account(
        mut,
        seeds = [TOURNAMENT_PLAYERS_SEED, &[1u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_1: Option<Box<Account<'info, TournamentPlayersShard>>>,
    /// Shard 2 — present for 256-player tournaments only.
    #[account(
        mut,
        seeds = [TOURNAMENT_PLAYERS_SEED, &[2u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_2: Option<Box<Account<'info, TournamentPlayersShard>>>,
    /// Shard 3 — present for 256-player tournaments only.
    #[account(
        mut,
        seeds = [TOURNAMENT_PLAYERS_SEED, &[3u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_3: Option<Box<Account<'info, TournamentPlayersShard>>>,
    /// CHECK: Operator treasury — receives the platform fee cut (£0.50 equivalent per entry).
    /// Must match tournament.host_treasury set at initialize.
    #[account(
        mut,
        constraint = host_treasury.key() == tournament.host_treasury @ GameErrorCode::UnauthorizedAccess
    )]
    pub host_treasury: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<RegisterPlayer>, tournament_id: u64, elo: u32) -> Result<()> {
    let tournament = &mut ctx.accounts.tournament;
    let player = ctx.accounts.player.key();

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

    // Check for duplicate registration across all present shards.
    // Shards that weren't initialized (None) are skipped — they hold no players.
    macro_rules! check_dup {
        ($shard:expr) => {
            for existing in $shard.players.iter() {
                require!(*existing != player, GameErrorCode::AlreadyRegistered);
            }
        };
    }
    check_dup!(ctx.accounts.tournament_players_shard_0);
    if let Some(s) = ctx.accounts.tournament_players_shard_1.as_ref() { check_dup!(s); }
    if let Some(s) = ctx.accounts.tournament_players_shard_2.as_ref() { check_dup!(s); }
    if let Some(s) = ctx.accounts.tournament_players_shard_3.as_ref() { check_dup!(s); }

    // Determine which shard this player slots into.
    let shard_id = (tournament.num_registered_players / TournamentPlayersShard::SHARD_CAPACITY) as u8;

    match shard_id {
        0 => {
            let s = &mut ctx.accounts.tournament_players_shard_0;
            s.players.push(player);
            s.player_elos.push(elo);
        }
        1 => {
            let s = ctx.accounts.tournament_players_shard_1.as_mut()
                .ok_or(GameErrorCode::TournamentFull)?;
            s.players.push(player);
            s.player_elos.push(elo);
        }
        2 => {
            let s = ctx.accounts.tournament_players_shard_2.as_mut()
                .ok_or(GameErrorCode::TournamentFull)?;
            s.players.push(player);
            s.player_elos.push(elo);
        }
        3 => {
            let s = ctx.accounts.tournament_players_shard_3.as_mut()
                .ok_or(GameErrorCode::TournamentFull)?;
            s.players.push(player);
            s.player_elos.push(elo);
        }
        _ => return Err(GameErrorCode::TournamentFull.into()),
    }

    tournament.num_registered_players += 1;

    // Split entry fee:
    //   wager_contribution (entry_fee - platform_fee) → prize escrow (locked until prizes paid)
    //   platform_fee                                  → host_treasury (operational income)
    if tournament.entry_fee > 0 {
        let wager_contribution = tournament.entry_fee
            .saturating_sub(tournament.platform_fee);
        let fee_cut = tournament.platform_fee;

        if wager_contribution > 0 {
            anchor_lang::system_program::transfer(
                CpiContext::new(
                    ctx.accounts.system_program.to_account_info(),
                    anchor_lang::system_program::Transfer {
                        from: ctx.accounts.player.to_account_info(),
                        to: ctx.accounts.escrow_pda.to_account_info(),
                    },
                ),
                wager_contribution,
            )?;
            tournament.prize_pool += wager_contribution;
        }

        if fee_cut > 0 {
            anchor_lang::system_program::transfer(
                CpiContext::new(
                    ctx.accounts.system_program.to_account_info(),
                    anchor_lang::system_program::Transfer {
                        from: ctx.accounts.player.to_account_info(),
                        to: ctx.accounts.host_treasury.to_account_info(),
                    },
                ),
                fee_cut,
            )?;
            tournament.platform_fee_pool += fee_cut;
        }
    }

    msg!("Player {} registered with ELO {} in shard {}", player, elo, shard_id);
    Ok(())
}
