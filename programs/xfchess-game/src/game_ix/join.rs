use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(game_id: u64)]
pub struct JoinGame<'info> {
    #[account(mut, seeds = [GAME_SEED, &game_id.to_le_bytes()], bump)]
    pub game: Account<'info, Game>,
    /// CHECK: Escrow PDA
    #[account(mut, seeds = [WAGER_ESCROW_SEED, &game_id.to_le_bytes()], bump)]
    pub escrow_pda: UncheckedAccount<'info>,
    /// Player profile — created on first game if it doesn't exist yet.
    #[account(
        init_if_needed,
        payer = player,
        space = 8 + PlayerProfile::INIT_SPACE,
        seeds = [PROFILE_SEED, player.key().as_ref()],
        bump
    )]
    pub player_profile: Account<'info, PlayerProfile>,
    #[account(mut)]
    pub player: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<JoinGame>, _game_id: u64) -> Result<()> {
    let game = &mut ctx.accounts.game;
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

    game.black = ctx.accounts.player.key();
    game.status = GameStatus::Active;
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

    // Bootstrap profile if this is the joiner's first game.
    let profile = &mut ctx.accounts.player_profile;
    if profile.elo == 0 {
        profile.authority = ctx.accounts.player.key();
        profile.elo = 1200;
    }

    msg!("Player joined game. Match started!");
    Ok(())
}
