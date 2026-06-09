//! Instruction allowing players to voluntarily leave a tournament before it starts and receive a refund.
//! The entry fee is refunded from the tournament escrow PDA — the operator's wallet is not involved.

use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(tournament_id: u64)]
pub struct LeaveTournament<'info> {
    #[account(
        mut,
        seeds = [TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        bump = tournament.bump
    )]
    pub tournament: Account<'info, Tournament>,
    /// TournamentPlayersShard 0 (players 0-63)
    #[account(
        seeds = [TOURNAMENT_PLAYERS_SEED, &[0u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_0: Account<'info, TournamentPlayersShard>,
    /// TournamentPlayersShard 1 (players 64-127)
    #[account(
        seeds = [TOURNAMENT_PLAYERS_SEED, &[1u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_1: Account<'info, TournamentPlayersShard>,
    /// TournamentPlayersShard 2 (players 128-191)
    #[account(
        seeds = [TOURNAMENT_PLAYERS_SEED, &[2u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_2: Account<'info, TournamentPlayersShard>,
    /// TournamentPlayersShard 3 (players 192-255)
    #[account(
        seeds = [TOURNAMENT_PLAYERS_SEED, &[3u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_3: Account<'info, TournamentPlayersShard>,
    #[account(mut)]
    pub player: Signer<'info>,
    /// CHECK: Tournament escrow PDA — entry fees are held here, not in the operator's wallet.
    #[account(
        mut,
        seeds = [TOURNAMENT_ESCROW_SEED, &tournament_id.to_le_bytes()],
        bump
    )]
    pub escrow_pda: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<LeaveTournament>, tournament_id: u64) -> Result<()> {
    let tournament = &mut ctx.accounts.tournament;
    require!(tournament.tournament_id == tournament_id, GameErrorCode::UnauthorizedAccess);
    let player_key = ctx.accounts.player.key();

    // Validate tournament state
    require!(
        tournament.status == TournamentStatus::Registration,
        GameErrorCode::InvalidTournamentStatus
    );

    // Find the player's shard and index
    let mut found_shard_id: Option<u8> = None;
    let mut player_index_in_shard: Option<usize> = None;

    let shards = [
        (&mut ctx.accounts.tournament_players_shard_0, 0u8),
        (&mut ctx.accounts.tournament_players_shard_1, 1u8),
        (&mut ctx.accounts.tournament_players_shard_2, 2u8),
        (&mut ctx.accounts.tournament_players_shard_3, 3u8),
    ];

    for (shard, shard_id) in shards.iter() {
        for (i, player) in shard.players.iter().enumerate() {
            if *player == player_key {
                found_shard_id = Some(*shard_id);
                player_index_in_shard = Some(i);
                break;
            }
        }
        if found_shard_id.is_some() {
            break;
        }
    }

    let (shard_id, index) = match (found_shard_id, player_index_in_shard) {
        (Some(sid), Some(idx)) => (sid, idx),
        _ => return Err(GameErrorCode::PlayerNotFound.into()),
    };

    // Get mutable reference to the correct shard
    let target_shard = match shard_id {
        0 => &mut ctx.accounts.tournament_players_shard_0,
        1 => &mut ctx.accounts.tournament_players_shard_1,
        2 => &mut ctx.accounts.tournament_players_shard_2,
        3 => &mut ctx.accounts.tournament_players_shard_3,
        _ => return Err(GameErrorCode::PlayerNotFound.into()),
    };

    // Remove player and their ELO by shifting the array left
    let num_in_shard = target_shard.players.len();
    for i in index..(num_in_shard - 1) {
        target_shard.players[i] = target_shard.players[i + 1];
        target_shard.player_elos[i] = target_shard.player_elos[i + 1];
    }

    // Clear the last element (optional but clean)
    target_shard.players[num_in_shard - 1] = Pubkey::default();
    target_shard.player_elos[num_in_shard - 1] = 0;

    // Decrement the player count
    tournament.num_registered_players -= 1;

    // Refund entry fee from the tournament escrow PDA (direct lamport manipulation,
    // same pattern as the wager escrow in finalize.rs).
    let refund_amount = tournament.entry_fee;
    if refund_amount > 0 {
        require!(
            ctx.accounts.escrow_pda.lamports() >= refund_amount,
            GameErrorCode::InsufficientTreasuryForRefund
        );
        **ctx.accounts.escrow_pda.lamports.borrow_mut() -= refund_amount;
        **ctx.accounts.player.lamports.borrow_mut() += refund_amount;
    }

    // Update prize pool
    if tournament.prize_pool >= tournament.entry_fee {
        tournament.prize_pool -= tournament.entry_fee;
    }

    Ok(())
}
