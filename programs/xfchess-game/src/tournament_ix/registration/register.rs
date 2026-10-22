use crate::constants::*;
use crate::elo::rating::display_rating;
use crate::errors::GameErrorCode;
use crate::state::*;
use crate::tournament_ix::lifecycle::initialize_escrow::TournamentEscrow;
use crate::tournament_ix::registration::pass_accounts;
use crate::tournament_ix::shards;
use anchor_lang::prelude::*;
use anchor_spl::token_2022::Token2022;
use anchor_spl::token_interface::{self, Burn, Mint, TokenAccount};

#[derive(Accounts)]
#[instruction(tournament_id: u64)]
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
    #[account(
        mut,
        seeds = [TOURNAMENT_ESCROW_SEED, &tournament_id.to_le_bytes()],
        bump
    )]
    pub escrow_pda: Account<'info, TournamentEscrow>,
    #[account(
        mut,
        seeds = [TOURNAMENT_PLAYERS_SEED, &[0u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_0: Box<Account<'info, TournamentPlayersShard>>,
    #[account(
        mut,
        seeds = [TOURNAMENT_PLAYERS_SEED, &[1u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_1: Option<Box<Account<'info, TournamentPlayersShard>>>,
    #[account(
        mut,
        seeds = [TOURNAMENT_PLAYERS_SEED, &[2u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_2: Option<Box<Account<'info, TournamentPlayersShard>>>,
    #[account(
        mut,
        seeds = [TOURNAMENT_PLAYERS_SEED, &[3u8], &tournament_id.to_le_bytes()],
        bump
    )]
    pub tournament_players_shard_3: Option<Box<Account<'info, TournamentPlayersShard>>>,
    #[account(
        mut,
        constraint = host_treasury.key() == tournament.host_treasury @ GameErrorCode::UnauthorizedAccess,
        constraint = host_treasury.owner == &system_program::ID @ GameErrorCode::InvalidAccountOwner
    )]
    pub host_treasury: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,

    /// CHECK: fixed PDA, so it cannot be omitted or substituted. Empty and system-owned unless
    /// the organizer bound a pass mint to this tournament; `load_pass_binding` checks the rest.
    #[account(
        seeds = [TOURNAMENT_PASS_BINDING_SEED, &tournament_id.to_le_bytes()],
        bump
    )]
    pub pass_binding: UncheckedAccount<'info>,
    /// Required only for tournaments with a pass binding; checked against it in the handler.
    #[account(mut)]
    pub pass_mint: Option<Box<InterfaceAccount<'info, Mint>>>,
    #[account(mut)]
    pub player_pass_account: Option<Box<InterfaceAccount<'info, TokenAccount>>>,
    pub token_program_2022: Option<Program<'info, Token2022>>,
}

pub fn handler(ctx: Context<RegisterPlayer>, tournament_id: u64) -> Result<()> {
    let tournament = &mut ctx.accounts.tournament;
    require!(
        tournament.tournament_id == tournament_id,
        GameErrorCode::UnauthorizedAccess
    );
    let player = ctx.accounts.player.key();

    require!(
        tournament.status == TournamentStatus::Registration,
        GameErrorCode::InvalidTournamentStatus
    );
    require!(
        tournament.num_registered_players < tournament.max_players,
        GameErrorCode::TournamentFull
    );
    if tournament.entry_fee > 0 {
        require!(
            tournament.prize_pool > 0 || tournament.usdc_prize_funded,
            GameErrorCode::PrizeNotFunded
        );
    }
    // The rating comes from the player's own profile, never from the caller.
    let elo = display_rating(&ctx.accounts.player_profile, tournament.base_time_seconds);
    require!(
        tournament.elo_min <= elo && elo <= tournament.elo_max,
        GameErrorCode::EloOutOfRange
    );

    let mut present_shards: Vec<&TournamentPlayersShard> =
        vec![&ctx.accounts.tournament_players_shard_0];
    if let Some(s) = ctx.accounts.tournament_players_shard_1.as_ref() {
        present_shards.push(s);
    }
    if let Some(s) = ctx.accounts.tournament_players_shard_2.as_ref() {
        present_shards.push(s);
    }
    if let Some(s) = ctx.accounts.tournament_players_shard_3.as_ref() {
        present_shards.push(s);
    }
    require!(
        !shards::contains_player(&present_shards, player),
        GameErrorCode::AlreadyRegistered
    );

    redeem_pass(
        &ctx.accounts.pass_binding,
        tournament_id,
        &ctx.accounts.player,
        ctx.accounts.pass_mint.as_deref(),
        ctx.accounts.player_pass_account.as_deref(),
        ctx.accounts.token_program_2022.as_ref(),
        ctx.remaining_accounts,
    )?;

    let shard_id =
        (tournament.num_registered_players / TournamentPlayersShard::SHARD_CAPACITY) as u8;

    match shard_id {
        0 => {
            let s = &mut ctx.accounts.tournament_players_shard_0;
            shards::push_player(s, player, elo)?;
        }
        1 => {
            let s = ctx
                .accounts
                .tournament_players_shard_1
                .as_mut()
                .ok_or(GameErrorCode::TournamentFull)?;
            shards::push_player(s, player, elo)?;
        }
        2 => {
            let s = ctx
                .accounts
                .tournament_players_shard_2
                .as_mut()
                .ok_or(GameErrorCode::TournamentFull)?;
            shards::push_player(s, player, elo)?;
        }
        3 => {
            let s = ctx
                .accounts
                .tournament_players_shard_3
                .as_mut()
                .ok_or(GameErrorCode::TournamentFull)?;
            shards::push_player(s, player, elo)?;
        }
        _ => return Err(GameErrorCode::TournamentFull.into()),
    }

    tournament.num_registered_players = tournament
        .num_registered_players
        .checked_add(1)
        .ok_or(GameErrorCode::ArithmeticOverflow)?;
    tournament.player_count = tournament
        .player_count
        .checked_add(1)
        .ok_or(GameErrorCode::ArithmeticOverflow)?;

    if tournament.entry_fee > 0 {
        anchor_lang::system_program::transfer(
            CpiContext::new(
                System::id(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.player.to_account_info(),
                    to: ctx.accounts.escrow_pda.to_account_info(),
                },
            ),
            tournament.entry_fee,
        )?;
    }

    msg!(
        "Player {} registered with ELO {} in shard {}",
        player,
        elo,
        shard_id
    );
    Ok(())
}

/// `None` when the tournament has no pass binding. Only an empty system-owned account counts as
/// absent: its address is a fixed PDA, so a caller cannot fake or omit a real binding.
fn load_pass_binding(
    pass_binding: &AccountInfo,
    tournament_id: u64,
) -> Result<Option<TournamentPassBinding>> {
    if pass_binding.owner == &System::id() && pass_binding.data_is_empty() {
        return Ok(None);
    }
    require_keys_eq!(
        *pass_binding.owner,
        crate::ID,
        GameErrorCode::InvalidAccountOwner
    );
    let binding = TournamentPassBinding::try_deserialize(&mut &pass_binding.try_borrow_data()?[..])?;
    require!(
        binding.tournament_id == tournament_id,
        GameErrorCode::UnauthorizedAccess
    );
    Ok(Some(binding))
}

/// Burns the player's pass when the tournament is bound to one, so a pass admits one
/// registration and cannot be handed on after use. Without a binding this does nothing.
///
/// The first remaining account must be the player's eligibility record in the pass program, which
/// only this program's `issue_pass_eligibility` can create. Holding a pass is not enough: a pass
/// that reached the wallet without going through the hook (minted before the authority was
/// revoked) or whose record was revoked cannot register.
fn redeem_pass<'info>(
    pass_binding: &AccountInfo<'info>,
    tournament_id: u64,
    player: &Signer<'info>,
    pass_mint: Option<&InterfaceAccount<'info, Mint>>,
    player_pass_account: Option<&InterfaceAccount<'info, TokenAccount>>,
    token_program: Option<&Program<'info, Token2022>>,
    remaining_accounts: &[AccountInfo<'info>],
) -> Result<()> {
    let Some(binding) = load_pass_binding(pass_binding, tournament_id)? else {
        return Ok(());
    };
    let (Some(pass_mint), Some(player_pass_account), Some(token_program)) =
        (pass_mint, player_pass_account, token_program)
    else {
        return err!(GameErrorCode::PassRequired);
    };
    require!(
        pass_mint.key() == binding.pass_mint
            && *pass_mint.to_account_info().owner == anchor_spl::token_2022::ID,
        GameErrorCode::PassMintMismatch
    );
    require!(
        player_pass_account.mint == pass_mint.key()
            && player_pass_account.owner == player.key()
            && player_pass_account.amount >= 1,
        GameErrorCode::PassNotHeld
    );
    let (expected_record, _) = Pubkey::find_program_address(
        &[b"eligibility", binding.pass_mint.as_ref(), player.key().as_ref()],
        &XFCHESS_PASS_PROGRAM_ID,
    );
    let Some(record_info) = remaining_accounts
        .first()
        .filter(|account| account.key() == expected_record)
    else {
        return err!(GameErrorCode::PassEligibilityMissing);
    };
    let record = pass_accounts::load_record(record_info)?;
    pass_accounts::check_record(record.as_ref(), &binding.pass_mint, &player.key())?;
    token_interface::burn(
        CpiContext::new(
            token_program.key(),
            Burn {
                mint: pass_mint.to_account_info(),
                from: player_pass_account.to_account_info(),
                authority: player.to_account_info(),
            },
        ),
        1,
    )
}
