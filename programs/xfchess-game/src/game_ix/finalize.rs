use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(game_id: u64)]
pub struct EndGame<'info> {
    #[account(mut, seeds = [GAME_SEED, &game_id.to_le_bytes()], bump)]
    pub game: Account<'info, Game>,
    #[account(mut, seeds = [PROFILE_SEED, game.white.as_ref()], bump)]
    pub white_profile: Account<'info, PlayerProfile>,
    #[account(mut, seeds = [PROFILE_SEED, game.black.as_ref()], bump)]
    pub black_profile: Account<'info, PlayerProfile>,
    /// CHECK: Destination for white winnings
    #[account(mut)]
    pub white_authority: UncheckedAccount<'info>,
    /// CHECK: Destination for black winnings
    #[account(mut)]
    pub black_authority: UncheckedAccount<'info>,
    /// CHECK: Wager escrow PDA
    #[account(mut, seeds = [WAGER_ESCROW_SEED, &game_id.to_le_bytes()], bump)]
    pub escrow_pda: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<EndGame>, _game_id: u64, result: GameResult) -> Result<()> {
    let game = &mut ctx.accounts.game;
    require!(
        game.status == GameStatus::Active,
        GameErrorCode::GameNotActive
    );

    game.status = GameStatus::Finished;
    game.result = result.clone();
    game.updated_at = Clock::get()?.unix_timestamp;

    // Extract copies before releasing the mutable borrow for CPI calls
    let wager_amount = game.wager_amount;
    let game_white = game.white;
    let wager_token = game.wager_token;

    // --- SOL Payout via invoke_signed ---
    // The escrow PDA is system-program-owned (funded by system_program::transfer in
    // create_game/join_game). Direct lamport reduction requires program ownership, so we
    // use CpiContext::new_with_signer to let the system program process the transfer using
    // the escrow PDA seeds as the signer proof.
    if wager_amount > 0 && wager_token.is_none() {
        let pot = wager_amount * 2;
        let game_id_bytes = _game_id.to_le_bytes();
        let bump = ctx.bumps.escrow_pda;
        let escrow_seeds: &[&[&[u8]]] = &[&[WAGER_ESCROW_SEED, &game_id_bytes, &[bump]]];

        match result {
            GameResult::Winner(winner) => {
                let dest = if winner == game_white {
                    ctx.accounts.white_authority.to_account_info()
                } else {
                    ctx.accounts.black_authority.to_account_info()
                };
                anchor_lang::system_program::transfer(
                    CpiContext::new_with_signer(
                        ctx.accounts.system_program.to_account_info(),
                        anchor_lang::system_program::Transfer {
                            from: ctx.accounts.escrow_pda.to_account_info(),
                            to: dest,
                        },
                        escrow_seeds,
                    ),
                    pot,
                )?;
            }
            GameResult::Draw => {
                let half = wager_amount;
                anchor_lang::system_program::transfer(
                    CpiContext::new_with_signer(
                        ctx.accounts.system_program.to_account_info(),
                        anchor_lang::system_program::Transfer {
                            from: ctx.accounts.escrow_pda.to_account_info(),
                            to: ctx.accounts.white_authority.to_account_info(),
                        },
                        escrow_seeds,
                    ),
                    half,
                )?;
                anchor_lang::system_program::transfer(
                    CpiContext::new_with_signer(
                        ctx.accounts.system_program.to_account_info(),
                        anchor_lang::system_program::Transfer {
                            from: ctx.accounts.escrow_pda.to_account_info(),
                            to: ctx.accounts.black_authority.to_account_info(),
                        },
                        escrow_seeds,
                    ),
                    half,
                )?;
            }
            _ => {}
        }
    }

    // --- ELO Updates ---
    let k = 32.0;
    let white_profile = &mut ctx.accounts.white_profile;
    let black_profile = &mut ctx.accounts.black_profile;

    let ea =
        1.0 / (1.0 + 10.0f64.powf((black_profile.elo as f64 - white_profile.elo as f64) / 400.0));
    let eb = 1.0 - ea;

    let (sa, sb) = match result {
        GameResult::Winner(w) => {
            if w == game_white {
                (1.0, 0.0)
            } else {
                (0.0, 1.0)
            }
        }
        GameResult::Draw => (0.5, 0.5),
        _ => (0.5, 0.5),
    };

    white_profile.elo = (white_profile.elo as f64 + k * (sa - ea)) as u16;
    black_profile.elo = (black_profile.elo as f64 + k * (sb - eb)) as u16;

    white_profile.games_played += 1;
    black_profile.games_played += 1;

    match result {
        GameResult::Winner(w) => {
            if w == game_white {
                white_profile.wins += 1;
                black_profile.losses += 1;
            } else {
                black_profile.wins += 1;
                white_profile.losses += 1;
            }
        }
        GameResult::Draw => {
            white_profile.draws += 1;
            black_profile.draws += 1;
        }
        _ => {}
    }

    Ok(())
}
