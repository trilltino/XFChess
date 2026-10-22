use anyhow::Context as _;
pub mod common;

use anchor_lang::Space;
use common::*;
use solana_sdk::{account::AccountSharedData, clock::Clock, pubkey::Pubkey, signer::Signer};
use xfchess_game::errors::GameErrorCode;
use xfchess_game::state::{Game, GameResult, GameStatus, GameType, MatchType};

const GAME_ID: u64 = 90_001;
// A timed game may only be ended on time once the side to move's whole clock
// (base time plus earned increments; no increment here) has run out.
const BASE_TIME_SECONDS: u64 = 300;

fn timed_active_game(
    white: Pubkey,
    black: Pubkey,
    turn: u16,
    updated_at: i64,
) -> anyhow::Result<(Pubkey, solana_sdk::account::Account)> {
    Ok({
        let (pda, bump) = game_pda(GAME_ID);
        let game = Game {
            game_id: GAME_ID,
            white,
            black,
            status: GameStatus::Active,
            last_move_timestamp: updated_at,
            fees_advanced: 0,
            fee_payer: white,
            result: GameResult::None,
            board_state: start_board(),
            move_count: turn.saturating_sub(1),
            halfmove_clock: 0,
            turn,
            created_at: updated_at,
            updated_at,
            wager_amount: 0,
            wager_token: None,
            game_type: GameType::PvP,
            match_type: MatchType::Free,
            country_fee: 0,
            base_time_seconds: BASE_TIME_SECONDS,
            increment_seconds: 0,
            bump,
            is_delegated: false,
            tournament_id: None,
            nonce: 0,
            draw_offered_by: None,
        };
        (pda, program_account(&game, 8 + Game::INIT_SPACE)?)
    })
}

#[tokio::test]
async fn claim_timeout_rejects_before_inactivity_window_elapses() -> anyhow::Result<()> {
    let white = Pubkey::new_unique();
    let black = Pubkey::new_unique();

    let mut ctx = start(vec![]).await;
    let now = ctx
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .context("required operation value")?
        .unix_timestamp;

    // Only 30s elapsed since the last move; even the minimum timed-game
    // window (`TIMED_GAME_INACTIVITY_WINDOW_SECONDS`) is 90s.
    let (game_key, game_data) = timed_active_game(white, black, 1, now - 30)?;
    ctx.set_account(&game_key, &AccountSharedData::from(game_data));

    let ix = claim_timeout_ix(GAME_ID, ctx.payer.pubkey());
    let err = send(&mut ctx, ix, &[])
        .await
        .err()
        .context("claim_timeout must reject before the inactivity window elapses")?;

    assert_eq!(
        custom_code(&err),
        Some(ec(GameErrorCode::TimeoutNotExpired)),
        "expected TimeoutNotExpired, got {err:?}"
    );

    let game = fetch_game(&mut ctx, GAME_ID).await?;
    assert_eq!(
        game.status,
        GameStatus::Active,
        "a rejected timeout claim must not mutate the game"
    );
    Ok(())
}

#[tokio::test]
async fn claim_timeout_awards_white_when_black_flags() -> anyhow::Result<()> {
    let white = Pubkey::new_unique();
    let black = Pubkey::new_unique();

    let mut ctx = start(vec![]).await;
    let now = ctx
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .context("required operation value")?
        .unix_timestamp;

    // Even turn 2 expires black's clock: 400s exceeds black's whole 300s clock.
    let (game_key, game_data) = timed_active_game(white, black, 2, now - 400)?;
    ctx.set_account(&game_key, &AccountSharedData::from(game_data));

    let ix = claim_timeout_ix(GAME_ID, ctx.payer.pubkey());
    send(&mut ctx, ix, &[])
        .await
        .context("claim_timeout should succeed once the inactivity window has elapsed")?;

    let game = fetch_game(&mut ctx, GAME_ID).await?;
    assert_eq!(game.status, GameStatus::Finished);
    assert_eq!(
        game.result,
        GameResult::Winner(white),
        "black's clock expired (even turn) -> white must win"
    );
    Ok(())
}

#[tokio::test]
async fn claim_timeout_cancels_zero_move_game_instead_of_awarding_black() -> anyhow::Result<()> {
    let white = Pubkey::new_unique();
    let black = Pubkey::new_unique();

    let mut ctx = start(vec![]).await;
    let now = ctx
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .context("required operation value")?
        .unix_timestamp;

    let (game_key, game_data) = timed_active_game(white, black, 1, now - 200)?;
    ctx.set_account(&game_key, &AccountSharedData::from(game_data));

    let ix = claim_timeout_ix(GAME_ID, ctx.payer.pubkey());
    send(&mut ctx, ix, &[])
        .await
        .context("claim_timeout should cancel an expired zero-move game")?;

    let game = fetch_game(&mut ctx, GAME_ID).await?;
    assert_eq!(game.status, GameStatus::Cancelled);
    assert_eq!(
        game.result,
        GameResult::None,
        "zero moves means no winner and no one-sided payout"
    );
    Ok(())
}

#[tokio::test]
async fn claim_timeout_rejects_a_think_shorter_than_the_movers_clock() -> anyhow::Result<()> {
    let white = Pubkey::new_unique();
    let black = Pubkey::new_unique();

    let mut ctx = start(vec![]).await;
    let now = ctx
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .context("required operation value")?
        .unix_timestamp;

    // 200s on one move is legal in a 5-minute game; it must not be forfeitable.
    let (game_key, game_data) = timed_active_game(white, black, 2, now - 200)?;
    ctx.set_account(&game_key, &AccountSharedData::from(game_data));

    let ix = claim_timeout_ix(GAME_ID, ctx.payer.pubkey());
    let err = send(&mut ctx, ix, &[])
        .await
        .err()
        .context("a player with time left on the clock must not lose on time")?;
    assert_eq!(
        custom_code(&err),
        Some(ec(GameErrorCode::TimeoutNotExpired)),
        "expected TimeoutNotExpired, got {err:?}"
    );
    assert_eq!(
        fetch_game(&mut ctx, GAME_ID).await?.status,
        GameStatus::Active
    );
    Ok(())
}
