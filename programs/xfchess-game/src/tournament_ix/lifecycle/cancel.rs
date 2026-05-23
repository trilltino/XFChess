//! Instruction for safely halting a tournament and refunding entry fees.
//! For USDC tournaments: returns USDC to operator and refunds SOL entry fees from host_treasury.
//! Uses remaining_accounts for variable player count (up to 256).

use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};

#[derive(Accounts)]
#[instruction(tournament_id: u64)]
pub struct CancelTournament<'info> {
    #[account(
        mut,
        seeds = [TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        bump = tournament.bump,
        constraint = tournament.authority == authority.key() @ GameErrorCode::NotTournamentAuthority
    )]
    pub tournament: Account<'info, Tournament>,
    /// TournamentPlayersShard 0 (players 0-63)
    #[account(
        seeds = [TOURNAMENT_PLAYERS_SEED, &[0u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_0: Box<Account<'info, TournamentPlayersShard>>,
    /// TournamentPlayersShard 1 (players 64-127)
    #[account(
        seeds = [TOURNAMENT_PLAYERS_SEED, &[1u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_1: Box<Account<'info, TournamentPlayersShard>>,
    /// TournamentPlayersShard 2 (players 128-191)
    #[account(
        seeds = [TOURNAMENT_PLAYERS_SEED, &[2u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_2: Box<Account<'info, TournamentPlayersShard>>,
    /// TournamentPlayersShard 3 (players 192-255)
    #[account(
        seeds = [TOURNAMENT_PLAYERS_SEED, &[3u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_3: Box<Account<'info, TournamentPlayersShard>>,
    /// CHECK: USDC prize escrow PDA — the authority of the token account.
    #[account(
        seeds = [TOURNAMENT_USDC_PRIZE_SEED, &tournament_id.to_le_bytes()],
        bump
    )]
    pub usdc_prize_escrow_authority: UncheckedAccount<'info>,
    /// USDC prize escrow token account (only used if usdc_prize_mint is Some).
    #[account(
        mut,
        associated_token::mint = usdc_mint,
        associated_token::authority = usdc_prize_escrow_authority,
    )]
    pub usdc_prize_escrow: Option<Account<'info, TokenAccount>>,
    /// Operator's USDC ATA — receives returned USDC (only used if usdc_prize_mint is Some).
    #[account(mut)]
    pub operator_usdc_ata: Option<Account<'info, TokenAccount>>,
    /// The USDC mint account (only used if usdc_prize_mint is Some).
    pub usdc_mint: Option<Account<'info, token::Mint>>,
    /// Host treasury wallet — must sign to authorize SOL refunds.
    /// This is the operator's wallet that received entry fees.
    #[account(
        mut,
        constraint = host_treasury.key() == tournament.host_treasury @ GameErrorCode::UnauthorizedAccess
    )]
    pub host_treasury: Signer<'info>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn handler<'a, 'b, 'c, 'info>(ctx: Context<'a, 'b, 'c, 'info, CancelTournament<'info>>, tournament_id: u64) -> Result<()> {
    require!(
        ctx.accounts.tournament.status == TournamentStatus::Registration
            || ctx.accounts.tournament.status == TournamentStatus::Active,
        GameErrorCode::TournamentNotActive
    );

    let tournament = &ctx.accounts.tournament;
    let refund_amount = tournament.entry_fee;

    // Collect all players from all shards
    let mut all_players: Vec<Pubkey> = Vec::new();
    let shards = [
        &ctx.accounts.tournament_players_shard_0,
        &ctx.accounts.tournament_players_shard_1,
        &ctx.accounts.tournament_players_shard_2,
        &ctx.accounts.tournament_players_shard_3,
    ];

    for shard in shards.iter() {
        for player in shard.players.iter() {
            all_players.push(*player);
        }
    }

    let registered = all_players.len();

    // Step 1: Return USDC prize pool to operator (if funded)
    if tournament.usdc_prize_mint.is_some() && tournament.usdc_prize_funded {
        let usdc_prize_escrow = ctx.accounts.usdc_prize_escrow.as_ref()
            .ok_or(GameErrorCode::MissingTokenAccounts)?;
        let operator_usdc_ata = ctx.accounts.operator_usdc_ata.as_ref()
            .ok_or(GameErrorCode::MissingTokenAccounts)?;

        let usdc_balance = usdc_prize_escrow.amount;

        if usdc_balance > 0 {
            // Transfer USDC from escrow back to operator
            let tournament_id_bytes = tournament_id.to_le_bytes();
            let bump = ctx.bumps.usdc_prize_escrow_authority;
            let escrow_seeds: &[&[&[u8]]] = &[&[TOURNAMENT_USDC_PRIZE_SEED, &tournament_id_bytes, &[bump]]];

            token::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: usdc_prize_escrow.to_account_info(),
                        to: operator_usdc_ata.to_account_info(),
                        authority: ctx.accounts.usdc_prize_escrow_authority.to_account_info(),
                    },
                    escrow_seeds,
                ),
                usdc_balance,
            )?;

        }
    }

    // Check for duplicate player accounts to prevent double-refunds
    let mut seen_players = std::collections::HashSet::new();
    for player_key in all_players.iter() {
        require!(
            seen_players.insert(player_key),
            GameErrorCode::DuplicatePlayerAccount
        );
    }

    // Step 2: Refund entry fees to players from host_treasury
    if refund_amount > 0 && registered > 0 {
        // Use remaining_accounts for player wallets
        require!(
            ctx.remaining_accounts.len() >= registered,
            GameErrorCode::NotInGame
        );

        // Verify host_treasury has enough SOL for refunds
        let total_refund = refund_amount.checked_mul(registered as u64)
            .ok_or(GameErrorCode::Overflow)?;
        require!(
            ctx.accounts.host_treasury.lamports() >= total_refund,
            GameErrorCode::InsufficientTreasuryForRefund
        );

        for i in 0..registered {
            let player_key = all_players[i];
            let player_wallet = &ctx.remaining_accounts[i];
            require!(
                player_wallet.key() == player_key,
                GameErrorCode::NotInGame
            );
            require!(
                player_wallet.is_writable,
                GameErrorCode::UnauthorizedAccess
            );

            // Transfer from host_treasury to player
            anchor_lang::system_program::transfer(
                CpiContext::new(
                    ctx.accounts.system_program.to_account_info(),
                    anchor_lang::system_program::Transfer {
                        from: ctx.accounts.host_treasury.to_account_info(),
                        to: player_wallet.to_account_info(),
                    },
                ),
                refund_amount,
            )?;
        }

    }

    // Mark tournament as cancelled
    ctx.accounts.tournament.status = TournamentStatus::Cancelled;
    ctx.accounts.tournament.usdc_prize_funded = false;

    Ok(())
}
