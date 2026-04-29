//! Instruction allowing a second player to match the wager and join a game.

use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(game_id: u64)]
pub struct JoinGame<'info> {
    #[account(mut, seeds = [GAME_SEED, &game_id.to_le_bytes()], bump)]
    pub game: Account<'info, Game>,
    #[account(mut, seeds = [PROFILE_SEED, player.key().as_ref()], bump)]
    pub player_profile: Account<'info, PlayerProfile>,
    /// CHECK: PDA for escrowing SOL.
    #[account(mut, seeds = [WAGER_ESCROW_SEED, &game_id.to_le_bytes()], bump)]
    pub escrow_pda: UncheckedAccount<'info>,
    /// CHECK: White player profile for cross-border fee calculation
    #[account(seeds = [PROFILE_SEED, game.white.as_ref()], bump)]
    pub white_profile: Account<'info, PlayerProfile>,
    #[account(mut)]
    pub player: Signer<'info>,
    #[account(mut)]
    pub fee_payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

/// Get country fee based on country code and match type.
/// Returns 0 for Free games, otherwise returns the country-specific fee.
fn get_country_fee(country: &str, match_type: MatchType) -> u64 {
    if match_type == MatchType::Free {
        return 0;
    }
    
    match country {
        "GB" => UK_FEE_LAMPORTS,
        "BR" => BRAZIL_FEE_LAMPORTS,
        "CA" => CANADA_FEE_LAMPORTS,
        "DE" => GERMANY_FEE_LAMPORTS,
        _ => 0, // Default to 0 for unsupported countries
    }
}

/// Apply cross-border fee logic: if players are from different countries, use the lower fee.
fn apply_cross_border_fee_logic(
    white_country: &str,
    black_country: &str,
    white_fee: u64,
    black_fee: u64,
) -> u64 {
    if white_country != black_country {
        // Different countries - use the lower fee
        white_fee.min(black_fee)
    } else {
        // Same country - use the standard fee
        white_fee
    }
}

pub fn handler(ctx: Context<JoinGame>, _game_id: u64) -> Result<()> {
    let game = &mut ctx.accounts.game;
    let player = ctx.accounts.player.key();
    let player_country = &ctx.accounts.player_profile.country;
    let white_country = &ctx.accounts.white_profile.country;
    let fee_payer = ctx.accounts.fee_payer.key();

    require!(
        game.game_type == GameType::PvP,
        GameErrorCode::GameAlreadyFull
    ); // AI games are active by default
    require!(
        game.status == GameStatus::WaitingForOpponent,
        GameErrorCode::GameAlreadyFull
    );
    require!(
        game.white != ctx.accounts.player.key(),
        GameErrorCode::CannotPlaySelf
    );
    require!(
        game.fee_payer == fee_payer,
        GameErrorCode::FeePayerMismatch
    );

    // --- Cross-Border Fee Logic ---
    // Calculate fee for both countries
    let white_fee = get_country_fee(white_country, game.match_type.clone());
    let black_fee = get_country_fee(player_country, game.match_type.clone());
    
    // Apply cross-border logic: lower fee if countries differ
    let final_fee = apply_cross_border_fee_logic(white_country, player_country, white_fee, black_fee);
    
    // Update game's country fee to the cross-border adjusted fee
    game.country_fee = final_fee;

    game.black = player;
    game.status = GameStatus::Active;
    game.fees_advanced = game.fees_advanced.checked_add(CREATE_GAME_COST).ok_or(GameErrorCode::ArithmeticOverflow)?;
    game.updated_at = Clock::get()?.unix_timestamp;

    if game.wager_amount > 0 && game.wager_token.is_none() {
        anchor_lang::system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.player.to_account_info(),
                    to: ctx.accounts.escrow_pda.to_account_info(),
                },
            ),
            game.wager_amount,
        )?;
    }

    msg!("Player joined game. Match started with cross-border fee: {}", final_fee);
    Ok(())
}

pub fn join_game(
    ctx: Context<JoinGame>
) -> Result<()> {
    let game = &mut ctx.accounts.game;
    let joiner = &ctx.accounts.player;
    let fee_payer = &ctx.accounts.fee_payer;

    require!(game.status == GameStatus::WaitingForOpponent, GameErrorCode::GameNotActive);
    require!(game.black == Pubkey::default(), GameErrorCode::GameNotActive);
    require!(game.white != joiner.key(), GameErrorCode::GameNotActive);
    require!(game.fee_payer == fee_payer.key(), GameErrorCode::FeePayerMismatch);

    game.black = joiner.key();
    game.status = GameStatus::Active;
    game.fees_advanced = game.fees_advanced.checked_add(JOIN_GAME_COST).ok_or(GameErrorCode::ArithmeticOverflow)?;

    Ok(())
}
