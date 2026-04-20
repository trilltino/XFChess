use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};

#[derive(Accounts)]
#[instruction(game_id: u64)]
pub struct WithdrawExpiredWager<'info> {
    #[account(mut, seeds = [GAME_SEED, &game_id.to_le_bytes()], bump)]
    pub game: Account<'info, Game>,
    /// CHECK: Wager escrow PDA
    #[account(mut, seeds = [WAGER_ESCROW_SEED, &game_id.to_le_bytes()], bump)]
    pub escrow_pda: UncheckedAccount<'info>,
    #[account(mut)]
    pub player: Signer<'info>,
    pub system_program: Program<'info, System>,
    #[account(mut)]
    pub vault_nft_ata: Option<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub player_nft_ata: Option<Account<'info, TokenAccount>>,
    pub token_program: Option<Program<'info, Token>>,
}

pub fn handler(ctx: Context<WithdrawExpiredWager>, _game_id: u64) -> Result<()> {
    let game = &mut ctx.accounts.game;
    let player = ctx.accounts.player.key();

    require!(
        game.status == GameStatus::WaitingForOpponent,
        GameErrorCode::GameNotExpired
    );
    require!(game.white == player, GameErrorCode::NotGameCreator);

    let expiration_time = game.created_at + 86400;
    require!(
        Clock::get()?.unix_timestamp > expiration_time,
        GameErrorCode::GameNotExpired
    );

    if game.wager_amount > 0 {
        if let Some(_token_mint) = game.wager_token {
            // Unwrapping optionals for NFT/SPL transfer
            let vault_ata = ctx
                .accounts
                .vault_nft_ata
                .as_ref()
                .ok_or(GameErrorCode::MissingTokenAccounts)?;
            let player_ata = ctx
                .accounts
                .player_nft_ata
                .as_ref()
                .ok_or(GameErrorCode::MissingTokenAccounts)?;
            let token_program = ctx
                .accounts
                .token_program
                .as_ref()
                .ok_or(GameErrorCode::MissingTokenAccounts)?;

            let game_id_bytes = _game_id.to_le_bytes();
            let escrow_bump = ctx.bumps.escrow_pda;
            let seeds = &[WAGER_ESCROW_SEED, game_id_bytes.as_ref(), &[escrow_bump]];
            let signer_seeds = &[&seeds[..]];

            token::transfer(
                CpiContext::new_with_signer(
                    token_program.to_account_info(),
                    Transfer {
                        from: vault_ata.to_account_info(),
                        to: player_ata.to_account_info(),
                        authority: ctx.accounts.escrow_pda.to_account_info(),
                    },
                    signer_seeds,
                ),
                game.wager_amount,
            )?;
        } else {
            let pot = game.wager_amount;
            **ctx.accounts.escrow_pda.try_borrow_mut_lamports()? -= pot;
            **ctx.accounts.player.try_borrow_mut_lamports()? += pot;
        }
    }

    game.status = GameStatus::Expired;
    game.updated_at = Clock::get()?.unix_timestamp;

    msg!(
        "Expired wager for game {} withdrawn by {}",
        _game_id,
        player
    );
    Ok(())
}
