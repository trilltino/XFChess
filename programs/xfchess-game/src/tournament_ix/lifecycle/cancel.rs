use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use crate::tournament_ix::lifecycle::initialize_escrow::TournamentEscrow;
use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, TransferChecked};

#[derive(Accounts)]
#[instruction(tournament_id: u64)]
pub struct CancelTournament<'info> {
    #[account(
        mut,
        seeds = [TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        bump = tournament.bump,
        constraint = tournament.authority == authority.key() @ GameErrorCode::NotTournamentAuthority
    )]
    pub tournament: Box<Account<'info, Tournament>>,
    #[account(
        seeds = [TOURNAMENT_PLAYERS_SEED, &[0u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_0: Box<Account<'info, TournamentPlayersShard>>,
    #[account(
        seeds = [TOURNAMENT_PLAYERS_SEED, &[1u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_1: Option<Box<Account<'info, TournamentPlayersShard>>>,
    #[account(
        seeds = [TOURNAMENT_PLAYERS_SEED, &[2u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_2: Option<Box<Account<'info, TournamentPlayersShard>>>,
    #[account(
        seeds = [TOURNAMENT_PLAYERS_SEED, &[3u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_3: Option<Box<Account<'info, TournamentPlayersShard>>>,
    #[account(
        seeds = [TOURNAMENT_USDC_PRIZE_SEED, &tournament_id.to_le_bytes()],
        bump
    )]
    pub usdc_prize_escrow_authority: UncheckedAccount<'info>,
    #[account(
        mut,
        associated_token::mint = usdc_mint,
        associated_token::authority = usdc_prize_escrow_authority,
    )]
    pub usdc_prize_escrow: Option<Box<Account<'info, TokenAccount>>>,
    #[account(mut)]
    pub operator_usdc_ata: Option<Box<Account<'info, TokenAccount>>>,
    pub usdc_mint: Option<Box<Account<'info, token::Mint>>>,
    #[account(
        mut,
        seeds = [TOURNAMENT_ESCROW_SEED, &tournament_id.to_le_bytes()],
        bump
    )]
    pub escrow_pda: Box<Account<'info, TournamentEscrow>>,
    #[account(
        mut,
        constraint = host_treasury.key() == tournament.host_treasury @ GameErrorCode::UnauthorizedAccess
    )]
    pub host_treasury: Signer<'info>,
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(address = token::ID @ GameErrorCode::UnsupportedMintExtension)]
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[event]
pub struct TournamentCancelled {
    pub tournament_id: u64,
    pub authority: Pubkey,
    pub registered_players: u32,
    pub refund_amount_per_player: u64,
    pub sol_guarantee_returned: u64,
    pub usdc_prize_returned: u64,
    pub timestamp: i64,
}

pub fn handler<'info>(
    ctx: Context<'info, CancelTournament<'info>>,
    tournament_id: u64,
) -> Result<()> {
    require!(
        ctx.accounts.tournament.status == TournamentStatus::Registration
            || ctx.accounts.tournament.status == TournamentStatus::Active,
        GameErrorCode::TournamentNotActive
    );

    let tournament = &ctx.accounts.tournament;
    let refund_amount = tournament.entry_fee;
    let refund_from_escrow = tournament.status == TournamentStatus::Registration;
    let sol_guarantee = tournament.prize_pool;

    let mut all_players: Vec<Pubkey> = Vec::new();
    let mut shards: Vec<&TournamentPlayersShard> = vec![&ctx.accounts.tournament_players_shard_0];
    if let Some(s) = ctx.accounts.tournament_players_shard_1.as_ref() {
        shards.push(s);
    }
    if let Some(s) = ctx.accounts.tournament_players_shard_2.as_ref() {
        shards.push(s);
    }
    if let Some(s) = ctx.accounts.tournament_players_shard_3.as_ref() {
        shards.push(s);
    }

    for shard in shards.iter() {
        for player in shard.players.iter() {
            all_players.push(*player);
        }
    }

    let registered = all_players.len();

    if tournament.usdc_prize_mint.is_some() && tournament.usdc_prize_funded {
        let usdc_prize_escrow = ctx
            .accounts
            .usdc_prize_escrow
            .as_ref()
            .ok_or(GameErrorCode::MissingTokenAccounts)?;
        let operator_usdc_ata = ctx
            .accounts
            .operator_usdc_ata
            .as_ref()
            .ok_or(GameErrorCode::MissingTokenAccounts)?;
        let usdc_mint = ctx
            .accounts
            .usdc_mint
            .as_ref()
            .ok_or(GameErrorCode::MissingTokenAccounts)?;

        require_keys_eq!(
            usdc_mint.key(),
            tournament
                .usdc_prize_mint
                .ok_or(GameErrorCode::InvalidMint)?,
            GameErrorCode::InvalidMint
        );
        require_keys_eq!(
            operator_usdc_ata.mint,
            usdc_mint.key(),
            GameErrorCode::InvalidMint
        );
        require_keys_eq!(
            operator_usdc_ata.owner,
            ctx.accounts.authority.key(),
            GameErrorCode::UnauthorizedAccess
        );

        let usdc_balance = usdc_prize_escrow.amount;

        if usdc_balance > 0 {
            let tournament_id_bytes = tournament_id.to_le_bytes();
            let bump = ctx.bumps.usdc_prize_escrow_authority;
            let escrow_seeds: &[&[&[u8]]] =
                &[&[TOURNAMENT_USDC_PRIZE_SEED, &tournament_id_bytes, &[bump]]];

            token::transfer_checked(
                CpiContext::new_with_signer(
                    Token::id(),
                    TransferChecked {
                        from: usdc_prize_escrow.to_account_info(),
                        mint: usdc_mint.to_account_info(),
                        to: operator_usdc_ata.to_account_info(),
                        authority: ctx.accounts.usdc_prize_escrow_authority.to_account_info(),
                    },
                    escrow_seeds,
                ),
                usdc_balance,
                usdc_mint.decimals,
            )?;
        }
    }

    let mut seen_players = std::collections::HashSet::new();
    for player_key in all_players.iter() {
        require!(
            seen_players.insert(player_key),
            GameErrorCode::DuplicatePlayerAccount
        );
    }

    if refund_amount > 0 && registered > 0 {
        require!(
            ctx.remaining_accounts.len() == registered,
            GameErrorCode::InvalidRemainingAccounts
        );

        let total_refund = refund_amount
            .checked_mul(registered as u64)
            .ok_or(GameErrorCode::Overflow)?;
        let refund_source_balance = if refund_from_escrow {
            ctx.accounts.escrow_pda.to_account_info().lamports()
        } else {
            ctx.accounts.host_treasury.lamports()
        };
        require!(
            refund_source_balance >= total_refund,
            GameErrorCode::InsufficientTreasuryForRefund
        );

        for i in 0..registered {
            let player_key = all_players[i];
            let player_wallet = &ctx.remaining_accounts[i];
            require_keys_eq!(
                player_wallet.key(),
                player_key,
                GameErrorCode::InvalidRemainingAccounts
            );
            require!(
                player_wallet.is_writable && player_wallet.owner == &system_program::ID,
                GameErrorCode::InvalidRemainingAccounts
            );

            if refund_from_escrow {
                crate::common::escrow::debit_program_pda(
                    &ctx.accounts.escrow_pda.to_account_info(),
                    player_wallet,
                    refund_amount,
                )
                .map_err(|_| GameErrorCode::InsufficientTreasuryForRefund)?;
            } else {
                anchor_lang::system_program::transfer(
                    CpiContext::new(
                        System::id(),
                        anchor_lang::system_program::Transfer {
                            from: ctx.accounts.host_treasury.to_account_info(),
                            to: player_wallet.to_account_info(),
                        },
                    ),
                    refund_amount,
                )?;
            }
        }
    }

    if sol_guarantee > 0 {
        crate::common::escrow::debit_program_pda(
            &ctx.accounts.escrow_pda.to_account_info(),
            &ctx.accounts.host_treasury.to_account_info(),
            sol_guarantee,
        )?;
    }

    let usdc_prize_returned = ctx
        .accounts
        .usdc_prize_escrow
        .as_ref()
        .map(|a| a.amount)
        .unwrap_or(0);

    ctx.accounts.tournament.status = TournamentStatus::Cancelled;
    ctx.accounts.tournament.usdc_prize_funded = false;
    ctx.accounts.tournament.prize_pool = 0;

    emit!(TournamentCancelled {
        tournament_id,
        authority: ctx.accounts.authority.key(),
        registered_players: registered as u32,
        refund_amount_per_player: refund_amount,
        sol_guarantee_returned: sol_guarantee,
        usdc_prize_returned,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}
