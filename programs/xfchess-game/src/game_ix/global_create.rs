use crate::account_ix::session_guards;
use crate::common::escrow::debit_program_pda;
use crate::constants::{GAME_SEED, MAX_WAGER_AMOUNT, MIN_WAGER_LAMPORTS, WAGER_ESCROW_SEED};
use crate::errors::GameErrorCode;
use crate::game_ix::common::{init_game_fields, InitGameArgs};
use crate::state::{Game, GameResult, GameStatus, GameType, GlobalSessionDelegation, MatchType};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::invoke_signed;
use anchor_lang::solana_program::system_instruction;
use anchor_lang::Discriminator;

#[derive(Accounts)]
#[instruction(game_id: u64, wager_amount: u64, match_type: MatchType, platform_fee: u64, base_time_seconds: u64, increment_seconds: u16)]
pub struct GlobalCreateGame<'info> {
    #[account(
        mut,
        seeds = [GlobalSessionDelegation::SEED, player.key().as_ref()],
        bump = session_delegation.bump,
        constraint = session_delegation.session_key == session_signer.key() @ GameErrorCode::InvalidSessionKey,
        constraint = session_delegation.player == player.key() @ GameErrorCode::UnauthorizedAccess,
    )]
    pub session_delegation: Account<'info, GlobalSessionDelegation>,

    pub session_signer: Signer<'info>,

    pub player: UncheckedAccount<'info>,

    #[account(mut, seeds = [GAME_SEED, &game_id.to_le_bytes()], bump)]
    pub game: UncheckedAccount<'info>,

    #[account(mut, seeds = [WAGER_ESCROW_SEED, &game_id.to_le_bytes()], bump)]
    pub escrow_pda: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<GlobalCreateGame>,
    game_id: u64,
    wager_amount: u64,
    match_type: MatchType,
    platform_fee: u64,
    base_time_seconds: u64,
    increment_seconds: u16,
) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let session = &ctx.accounts.session_delegation;

    require!(
        session.is_valid(now),
        GameErrorCode::SessionExpiredOrDisabled
    );
    require!(
        session.games_remaining > 0,
        GameErrorCode::GlobalSessionNoGamesRemaining
    );
    require!(
        wager_amount <= MAX_WAGER_AMOUNT,
        GameErrorCode::WagerTooHigh
    );
    require!(
        wager_amount == 0 || wager_amount >= MIN_WAGER_LAMPORTS,
        GameErrorCode::StakeTooLow
    );
    require!(
        session.has_budget(wager_amount),
        GameErrorCode::GlobalSessionSpendingLimitExceeded
    );

    let space = 8 + Game::INIT_SPACE;
    let lamports = Rent::get()?.minimum_balance(space);

    {
        let vault = ctx.accounts.session_delegation.to_account_info();
        let rent_min = Rent::get()?.minimum_balance(vault.data_len());
        let required = lamports
            .checked_add(wager_amount)
            .ok_or(GameErrorCode::ArithmeticOverflow)?
            .checked_add(rent_min)
            .ok_or(GameErrorCode::ArithmeticOverflow)?;
        require!(
            vault.lamports() >= required,
            GameErrorCode::GlobalSessionVaultUnderfunded
        );
    }

    let game_id_bytes = game_id.to_le_bytes();
    let game_bump = [ctx.bumps.game];
    let game_seeds: [&[u8]; 3] = [GAME_SEED, game_id_bytes.as_ref(), game_bump.as_ref()];

    invoke_signed(
        &system_instruction::allocate(&ctx.accounts.game.key(), space as u64),
        &[
            ctx.accounts.game.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
        ],
        &[&game_seeds],
    )?;
    invoke_signed(
        &system_instruction::assign(&ctx.accounts.game.key(), ctx.program_id),
        &[
            ctx.accounts.game.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
        ],
        &[&game_seeds],
    )?;
    debit_program_pda(
        &ctx.accounts.session_delegation.to_account_info(),
        &ctx.accounts.game.to_account_info(),
        lamports,
    )?;

    debit_program_pda(
        &ctx.accounts.session_delegation.to_account_info(),
        &ctx.accounts.escrow_pda.to_account_info(),
        wager_amount,
    )?;

    let session = &mut ctx.accounts.session_delegation;
    session.total_spent = session_guards::checked_session_total(session.total_spent, wager_amount)?;
    session.games_remaining = session.games_remaining.saturating_sub(1);

    let mut game = Game {
        game_id: 0,
        white: Pubkey::default(),
        black: Pubkey::default(),
        status: GameStatus::Pending,
        last_move_timestamp: 0,
        fees_advanced: 0,
        fee_payer: Pubkey::default(),
        result: GameResult::None,
        board_state: [0; 68],
        move_count: 0,
        halfmove_clock: 0,
        turn: 0,
        created_at: 0,
        updated_at: 0,
        wager_amount: 0,
        wager_token: None,
        game_type: GameType::PvP,
        match_type: MatchType::Free,
        country_fee: 0,
        base_time_seconds: 0,
        increment_seconds: 0,
        bump: 0,
        is_delegated: false,
        tournament_id: None,
        nonce: 0,
        draw_offered_by: None,
    };
    init_game_fields(
        &mut game,
        InitGameArgs {
            game_id,
            white: ctx.accounts.player.key(),
            fee_payer: ctx.accounts.session_signer.key(),
            wager_amount,
            match_type,
            platform_fee,
            base_time_seconds,
            increment_seconds,
            tournament_id: None,
        },
        now,
        ctx.bumps.game,
    )?;

    let mut data = ctx.accounts.game.try_borrow_mut_data()?;
    data[..8].copy_from_slice(&Game::DISCRIMINATOR);
    let mut writer = &mut data[8..];
    game.serialize(&mut writer)?;

    Ok(())
}
