//! Base-layer settlement for finished, undelegated games.

use crate::common::escrow;
use crate::constants::*;
use crate::elo::glicko2::calculate_elo_update;
use crate::errors::GameErrorCode;
use crate::game_ix::finalize::EndGame;
use crate::lifecycle::guards;
use crate::state::*;
use anchor_lang::prelude::*;

pub fn settle_finished_game(ctx: Context<EndGame>, game_id: u64) -> Result<()> {
    let (result, wager_amount, game_white, wager_token, match_type, country_fee, fees_advanced) = {
        let game = &mut ctx.accounts.game;
        require!(
            game.status == GameStatus::Finished,
            GameErrorCode::GameNotFinished
        );
        guards::require_undelegated(game)?;
        require!(
            game.result != GameResult::None,
            GameErrorCode::GameStillInProgress
        );

        game.status = GameStatus::Settled;
        (
            game.result,
            game.wager_amount,
            game.white,
            game.wager_token,
            game.match_type,
            game.country_fee,
            game.fees_advanced,
        )
    };
    let bump = ctx.bumps.escrow_pda;

    let escrow_balance = ctx.accounts.escrow_pda.lamports();
    let pot = escrow::pot(wager_amount)?;

    if wager_amount > 0 && wager_token.is_none() && escrow_balance >= pot {
        let sp = &ctx.accounts.system_program;
        let escrow = &ctx.accounts.escrow_pda;
        let mut remaining = pot;

        let tx_fee = 10_000u64.min(remaining);
        escrow::pay_from_game_escrow(
            sp,
            escrow,
            ctx.accounts.fee_payer.as_ref(),
            tx_fee,
            game_id,
            bump,
        )?;
        remaining = remaining.saturating_sub(tx_fee);

        let platform_reimbursement = fees_advanced.min(remaining);
        escrow::pay_from_game_escrow(
            sp,
            escrow,
            ctx.accounts.treasury_vault.as_ref(),
            platform_reimbursement,
            game_id,
            bump,
        )?;
        remaining = remaining.saturating_sub(platform_reimbursement);

        if match_type != MatchType::Free {
            let cfee = country_fee.min(remaining);
            escrow::pay_from_game_escrow(
                sp,
                escrow,
                ctx.accounts.treasury_vault.as_ref(),
                cfee,
                game_id,
                bump,
            )?;
            remaining = remaining.saturating_sub(cfee);

            let per_player = (ELO_FEE_LAMPORTS / 2).min(remaining / 2);
            escrow::pay_from_game_escrow(
                sp,
                escrow,
                ctx.accounts.white_authority.as_ref(),
                per_player,
                game_id,
                bump,
            )?;
            escrow::pay_from_game_escrow(
                sp,
                escrow,
                ctx.accounts.black_authority.as_ref(),
                per_player,
                game_id,
                bump,
            )?;
            remaining = remaining.saturating_sub(per_player.saturating_mul(2));
        }

        match result {
            GameResult::Winner(winner) => {
                let dest = if winner == game_white {
                    ctx.accounts.white_authority.as_ref()
                } else {
                    ctx.accounts.black_authority.as_ref()
                };
                escrow::require_rent_exempt_after(dest, remaining)?;
                escrow::pay_from_game_escrow(sp, escrow, dest, remaining, game_id, bump)?;
            }
            GameResult::Draw => {
                let each = remaining / 2;
                escrow::require_rent_exempt_after(ctx.accounts.white_authority.as_ref(), each)?;
                escrow::require_rent_exempt_after(ctx.accounts.black_authority.as_ref(), each)?;
                escrow::pay_from_game_escrow(
                    sp,
                    escrow,
                    ctx.accounts.white_authority.as_ref(),
                    each,
                    game_id,
                    bump,
                )?;
                escrow::pay_from_game_escrow(
                    sp,
                    escrow,
                    ctx.accounts.black_authority.as_ref(),
                    each,
                    game_id,
                    bump,
                )?;
            }
            _ => {}
        }
    }

    update_profiles(ctx, result, game_white, wager_amount, match_type)
}

fn update_profiles(
    ctx: Context<EndGame>,
    result: GameResult,
    game_white: Pubkey,
    wager_amount: u64,
    match_type: MatchType,
) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let white_profile = &mut ctx.accounts.white_profile;
    let black_profile = &mut ctx.accounts.black_profile;

    if match_type != MatchType::Free {
        if white_profile.username.len() > 20 {
            white_profile.username = String::new();
            white_profile.username_set = false;
        }
        if black_profile.username.len() > 20 {
            black_profile.username = String::new();
            black_profile.username_set = false;
        }

        let sa = match result {
            GameResult::Winner(w) => {
                if w == game_white {
                    1.0
                } else {
                    0.0
                }
            }
            _ => 0.5,
        };
        let (new_w, new_b) =
            calculate_elo_update(white_profile.elo_rating, black_profile.elo_rating, sa);
        white_profile.elo_rating = new_w;
        black_profile.elo_rating = new_b;
        white_profile.last_played = now;
        black_profile.last_played = now;

        if let GameResult::Winner(w) = result {
            let (winner, loser) = if w == game_white {
                (&mut *white_profile, &mut *black_profile)
            } else {
                (&mut *black_profile, &mut *white_profile)
            };
            winner.win_streak = winner
                .win_streak
                .checked_add(1)
                .ok_or(GameErrorCode::ArithmeticOverflow)?;
            if winner.win_streak > winner.best_streak {
                winner.best_streak = winner.win_streak;
            }
            loser.win_streak = 0;
            winner.tournament_wins = winner
                .tournament_wins
                .checked_add(1)
                .ok_or(GameErrorCode::ArithmeticOverflow)?;
            match winner.country.as_str() {
                "GB" => {
                    winner.annual_wins_gbp = winner
                        .annual_wins_gbp
                        .checked_add(wager_amount)
                        .ok_or(GameErrorCode::ArithmeticOverflow)?;
                }
                "BR" => {
                    winner.annual_wins_brl = winner
                        .annual_wins_brl
                        .checked_add(wager_amount)
                        .ok_or(GameErrorCode::ArithmeticOverflow)?;
                }
                "CA" => {
                    winner.annual_wins_cad = winner
                        .annual_wins_cad
                        .checked_add(wager_amount)
                        .ok_or(GameErrorCode::ArithmeticOverflow)?;
                }
                "DE" => {
                    winner.annual_wins_eur = winner
                        .annual_wins_eur
                        .checked_add(wager_amount)
                        .ok_or(GameErrorCode::ArithmeticOverflow)?;
                }
                _ => {}
            }
        } else if result == GameResult::Draw {
            white_profile.win_streak = 0;
            black_profile.win_streak = 0;
        }

        white_profile.ranked_games = white_profile
            .ranked_games
            .checked_add(1)
            .ok_or(GameErrorCode::ArithmeticOverflow)?;
        black_profile.ranked_games = black_profile
            .ranked_games
            .checked_add(1)
            .ok_or(GameErrorCode::ArithmeticOverflow)?;
    }

    white_profile.games_played = white_profile
        .games_played
        .checked_add(1)
        .ok_or(GameErrorCode::ArithmeticOverflow)?;
    black_profile.games_played = black_profile
        .games_played
        .checked_add(1)
        .ok_or(GameErrorCode::ArithmeticOverflow)?;
    match result {
        GameResult::Winner(w) => {
            if w == game_white {
                white_profile.wins = white_profile
                    .wins
                    .checked_add(1)
                    .ok_or(GameErrorCode::ArithmeticOverflow)?;
                black_profile.losses = black_profile
                    .losses
                    .checked_add(1)
                    .ok_or(GameErrorCode::ArithmeticOverflow)?;
            } else {
                black_profile.wins = black_profile
                    .wins
                    .checked_add(1)
                    .ok_or(GameErrorCode::ArithmeticOverflow)?;
                white_profile.losses = white_profile
                    .losses
                    .checked_add(1)
                    .ok_or(GameErrorCode::ArithmeticOverflow)?;
            }
        }
        GameResult::Draw => {
            white_profile.draws = white_profile
                .draws
                .checked_add(1)
                .ok_or(GameErrorCode::ArithmeticOverflow)?;
            black_profile.draws = black_profile
                .draws
                .checked_add(1)
                .ok_or(GameErrorCode::ArithmeticOverflow)?;
        }
        _ => {}
    }

    Ok(())
}
