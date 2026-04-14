//! Instruction to finalize a completed game and distribute payouts.

use crate::constants::*;
use crate::errors::GameErrorCode;
use crate::state::*;
use crate::elo::glicko2::calculate_glicko2_update;
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
    /// CHECK: Treasury vault for collecting country fees (seeded by country code)
    /// Note: For now, we'll use a fixed treasury vault. In production, this should be derived per country.
    #[account(mut)]
    pub treasury_vault: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<EndGame>, _game_id: u64) -> Result<()> {
    let game = &mut ctx.accounts.game;
    require!(
        game.status == GameStatus::Finished,
        GameErrorCode::GameNotActive
    );

    // Result was already set on-chain by resign / claim_timeout / auto-checkmate detection.
    // Extract copies before releasing the mutable borrow for CPI calls
    let result = game.result.clone();
    let wager_amount = game.wager_amount;
    let game_white = game.white;
    let wager_token = game.wager_token;
    let match_type = game.match_type;
    let country_fee = game.country_fee;

    // --- SOL Payout via invoke_signed ---
    // The escrow PDA is system-program-owned (funded by system_program::transfer in
    // create_game/join_game). Direct lamport reduction requires program ownership, so we
    // use CpiContext::new_with_signer to let the system program process the transfer using
    // the escrow PDA seeds as the signer proof.
    // Only pay out if escrow still holds funds (resign/claim_timeout may have already paid)
    let escrow_balance = ctx.accounts.escrow_pda.lamports();
    let pot = wager_amount * 2;
    if wager_amount > 0 && wager_token.is_none() && escrow_balance >= pot {
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

    // --- Conditional ELO Updates and Fee Collection (Ranked/Wager/Tournament only) ---
    if match_type != MatchType::Free {
        let white_profile = &mut ctx.accounts.white_profile;
        let black_profile = &mut ctx.accounts.black_profile;

        // Handle legacy profiles (created before username fields were added)
        if white_profile.username.len() > 20 {
            white_profile.username = String::new();
            white_profile.username_set = false;
        }
        if black_profile.username.len() > 20 {
            black_profile.username = String::new();
            black_profile.username_set = false;
        }

        // --- Collect Treasury Fee ---
        if escrow_balance >= country_fee && country_fee > 0 {
            let game_id_bytes = _game_id.to_le_bytes();
            let escrow_bump = ctx.bumps.escrow_pda;
            let escrow_seeds: &[&[&[u8]]] = &[&[WAGER_ESCROW_SEED, &game_id_bytes, &[escrow_bump]]];
            
            // Transfer treasury fee from escrow to treasury vault
            // Note: treasury_vault is not a seeded account in current implementation
            // In production, this should be a PDA derived from country code
            anchor_lang::system_program::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.system_program.to_account_info(),
                    anchor_lang::system_program::Transfer {
                        from: ctx.accounts.escrow_pda.to_account_info(),
                        to: ctx.accounts.treasury_vault.to_account_info(),
                    },
                    escrow_seeds,
                ),
                country_fee,
            )?;
        }

        // --- Collect ELO Fee (split between players) ---
        let elo_fee_total = ELO_FEE_LAMPORTS;
        if escrow_balance >= country_fee + elo_fee_total && elo_fee_total > 0 {
            let game_id_bytes = _game_id.to_le_bytes();
            let escrow_bump = ctx.bumps.escrow_pda;
            let escrow_seeds: &[&[&[u8]]] = &[&[WAGER_ESCROW_SEED, &game_id_bytes, &[escrow_bump]]];
            
            // Split ELO fee between players (half each)
            let elo_fee_per_player = elo_fee_total / 2;
            
            // Transfer ELO fee to white authority
            anchor_lang::system_program::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.system_program.to_account_info(),
                    anchor_lang::system_program::Transfer {
                        from: ctx.accounts.escrow_pda.to_account_info(),
                        to: ctx.accounts.white_authority.to_account_info(),
                    },
                    escrow_seeds,
                ),
                elo_fee_per_player,
            )?;
            
            // Transfer ELO fee to black authority
            anchor_lang::system_program::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.system_program.to_account_info(),
                    anchor_lang::system_program::Transfer {
                        from: ctx.accounts.escrow_pda.to_account_info(),
                        to: ctx.accounts.black_authority.to_account_info(),
                    },
                    escrow_seeds,
                ),
                elo_fee_per_player,
            )?;
        }

        // --- Glicko-2 ELO Calculation ---
        let (sa, _sb) = match result {
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

        let w_rating = white_profile.elo_rating;
        let w_rd = white_profile.rd;
        let b_rating = black_profile.elo_rating;
        let b_rd = black_profile.rd;

        let (new_w_rating, new_w_rd, new_b_rating, new_b_rd) = 
            calculate_glicko2_update(w_rating, w_rd, b_rating, b_rd, sa);

        white_profile.elo_rating = new_w_rating;
        white_profile.rd = new_w_rd;
        black_profile.elo_rating = new_b_rating;
        black_profile.rd = new_b_rd;

        white_profile.last_played = Clock::get()?.unix_timestamp;
        black_profile.last_played = Clock::get()?.unix_timestamp;

        // --- Update Annual Wins for Compliance Reporting ---
        // Based on country and wager amount
        match white_profile.country.as_str() {
            "GB" => white_profile.annual_wins_gbp += wager_amount,
            "BR" => white_profile.annual_wins_brl += wager_amount,
            "CA" => white_profile.annual_wins_cad += wager_amount,
            "DE" => white_profile.annual_wins_eur += wager_amount,
            _ => {}
        }
        
        match black_profile.country.as_str() {
            "GB" => black_profile.annual_wins_gbp += wager_amount,
            "BR" => black_profile.annual_wins_brl += wager_amount,
            "CA" => black_profile.annual_wins_cad += wager_amount,
            "DE" => black_profile.annual_wins_eur += wager_amount,
            _ => {}
        }

        white_profile.ranked_games += 1;
        black_profile.ranked_games += 1;
    }

    // --- Update Game Stats (All Games) ---
    let white_profile = &mut ctx.accounts.white_profile;
    let black_profile = &mut ctx.accounts.black_profile;

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
