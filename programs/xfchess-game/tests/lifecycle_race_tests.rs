//! Test both serialized instruction orderings against the real program.
//! The losing instruction must leave state and lamports unchanged.

mod common;

use anchor_lang::{InstructionData, Space, ToAccountMetas};
use common::*;
use solana_program_test::ProgramTestContext;
use solana_sdk::{
    account::AccountSharedData, clock::Clock, instruction::Instruction, pubkey::Pubkey,
    signature::Keypair, signer::Signer,
};
use xfchess_game::errors::GameErrorCode;
use xfchess_game::state::{Game, GameResult, GameStatus, GameType, MatchType, PlayerProfile};

const WAGER: u64 = 50_000_000;
const PLAYER_FUNDS: u64 = 1_000_000_000;

fn escrow_pda(game_id: u64) -> Pubkey {
    Pubkey::find_program_address(&[b"escrow", &game_id.to_le_bytes()], &xfchess_game::ID).0
}

fn cancel_ix(game_id: u64, player: Pubkey, white: Pubkey, black: Pubkey) -> Instruction {
    let accounts = xfchess_game::__client_accounts_cancel_game::CancelGame {
        game: game_pda(game_id).0,
        escrow_pda: escrow_pda(game_id),
        player,
        white_authority: white,
        black_authority: black,
        system_program: solana_system_interface::program::ID,
    }
    .to_account_metas(None);
    // Same metas as the game client's `cancel_game_ix`: both authorities are
    // passed writable (the runtime demotes the unjoined default key itself).
    let accounts = accounts
        .into_iter()
        .map(|mut m| {
            if m.pubkey == black {
                m.is_writable = true;
            }
            m
        })
        .collect();
    Instruction {
        program_id: xfchess_game::ID,
        accounts,
        data: xfchess_game::instruction::CancelGame { game_id }.data(),
    }
}

fn join_ix(game_id: u64, player: Pubkey, white: Pubkey, fee_payer: Pubkey) -> Instruction {
    let accounts = xfchess_game::__client_accounts_join_game::JoinGame {
        game: game_pda(game_id).0,
        player_profile: profile_pda(&player).0,
        escrow_pda: escrow_pda(game_id),
        white_profile: profile_pda(&white).0,
        player,
        fee_payer,
        system_program: solana_system_interface::program::ID,
    }
    .to_account_metas(None);
    Instruction {
        program_id: xfchess_game::ID,
        accounts,
        data: xfchess_game::instruction::JoinGame { game_id }.data(),
    }
}

fn profile(authority: Pubkey) -> (Pubkey, solana_sdk::account::Account) {
    let p = PlayerProfile {
        authority,
        ..Default::default()
    };
    (
        profile_pda(&authority).0,
        program_account(&p, 8 + PlayerProfile::INIT_SPACE),
    )
}

#[allow(clippy::too_many_arguments)]
fn game(
    game_id: u64,
    white: Pubkey,
    black: Pubkey,
    fee_payer: Pubkey,
    status: GameStatus,
    move_count: u16,
    updated_at: i64,
) -> (Pubkey, solana_sdk::account::Account) {
    let (pda, bump) = game_pda(game_id);
    let g = Game {
        game_id,
        white,
        black,
        status,
        last_move_timestamp: updated_at,
        fees_advanced: 0,
        fee_payer,
        result: GameResult::None,
        board_state: start_board(),
        move_count,
        halfmove_clock: 0,
        turn: move_count + 1,
        created_at: updated_at,
        updated_at,
        wager_amount: WAGER,
        wager_token: None,
        game_type: GameType::PvP,
        match_type: MatchType::Free,
        country_fee: 0,
        base_time_seconds: 300,
        increment_seconds: 0,
        bump,
        is_delegated: false,
        tournament_id: None,
        nonce: move_count as u64,
        draw_offered_by: None,
    };
    (pda, program_account(&g, 8 + Game::INIT_SPACE))
}

async fn now(ctx: &mut ProgramTestContext) -> i64 {
    ctx.banks_client
        .get_sysvar::<Clock>()
        .await
        .unwrap()
        .unix_timestamp
}

async fn lamports(ctx: &mut ProgramTestContext, key: Pubkey) -> u64 {
    ctx.banks_client
        .get_account(key)
        .await
        .unwrap()
        .map(|a| a.lamports)
        .unwrap_or(0)
}

/// Advance to a fresh blockhash so a repeated identical instruction is a
/// distinct transaction rather than a deduplicated signature.
async fn next_blockhash(ctx: &mut ProgramTestContext) {
    ctx.last_blockhash = ctx.get_new_latest_blockhash().await.unwrap();
}

struct Table {
    ctx: ProgramTestContext,
    white: Keypair,
    black: Keypair,
}

/// A wagered game created by `white`, waiting for `black`; escrow funded with
/// white's stake. The test payer is the game's fee payer.
async fn waiting_table(game_id: u64) -> Table {
    let white = Keypair::new();
    let black = Keypair::new();
    let mut ctx = start(vec![
        (white.pubkey(), system_account(PLAYER_FUNDS)),
        (black.pubkey(), system_account(PLAYER_FUNDS)),
        profile(white.pubkey()),
        profile(black.pubkey()),
        (escrow_pda(game_id), system_account(WAGER)),
    ])
    .await;
    let t = now(&mut ctx).await;
    let (key, data) = game(
        game_id,
        white.pubkey(),
        Pubkey::default(),
        ctx.payer.pubkey(),
        GameStatus::WaitingForOpponent,
        0,
        t,
    );
    ctx.set_account(&key, &AccountSharedData::from(data));
    Table { ctx, white, black }
}

#[tokio::test]
async fn cancel_wins_the_race_then_join_fails_and_joiner_keeps_funds() {
    let id = 70_001;
    let Table {
        mut ctx,
        white,
        black,
    } = waiting_table(id).await;
    let white_before = lamports(&mut ctx, white.pubkey()).await;
    let black_before = lamports(&mut ctx, black.pubkey()).await;

    send(
        &mut ctx,
        cancel_ix(id, white.pubkey(), white.pubkey(), Pubkey::default()),
        &[&white],
    )
    .await
    .expect("creator cancels an unjoined game");
    assert_eq!(fetch_game(&mut ctx, id).await.status, GameStatus::Cancelled);
    assert_eq!(
        lamports(&mut ctx, white.pubkey()).await,
        white_before + WAGER
    );
    assert_eq!(lamports(&mut ctx, escrow_pda(id)).await, 0);

    let payer = ctx.payer.pubkey();
    let err = send(
        &mut ctx,
        join_ix(id, black.pubkey(), white.pubkey(), payer),
        &[&black],
    )
    .await
    .expect_err("join after cancellation must fail");
    assert_eq!(custom_code(&err), Some(ec(GameErrorCode::GameAlreadyFull)));
    assert_eq!(fetch_game(&mut ctx, id).await.status, GameStatus::Cancelled);
    assert_eq!(
        lamports(&mut ctx, black.pubkey()).await,
        black_before,
        "a rejected join moves no funds"
    );
    assert_eq!(lamports(&mut ctx, escrow_pda(id)).await, 0);
}

#[tokio::test]
async fn join_wins_the_race_then_creator_cancel_refunds_both_and_names_no_winner() {
    let id = 70_002;
    let Table {
        mut ctx,
        white,
        black,
    } = waiting_table(id).await;
    let white_before = lamports(&mut ctx, white.pubkey()).await;
    let black_before = lamports(&mut ctx, black.pubkey()).await;

    let payer = ctx.payer.pubkey();
    send(
        &mut ctx,
        join_ix(id, black.pubkey(), white.pubkey(), payer),
        &[&black],
    )
    .await
    .expect("join lands first");
    assert_eq!(lamports(&mut ctx, escrow_pda(id)).await, 2 * WAGER);

    // The creator's cancel arrives second: before any move it is the
    // zero-move cancellation — both stakes back, no result.
    send(
        &mut ctx,
        cancel_ix(id, white.pubkey(), white.pubkey(), black.pubkey()),
        &[&white],
    )
    .await
    .expect("zero-move cancellation after join");
    let g = fetch_game(&mut ctx, id).await;
    assert_eq!(g.status, GameStatus::Cancelled);
    assert_eq!(g.result, GameResult::None);
    assert_eq!(
        lamports(&mut ctx, white.pubkey()).await,
        white_before + WAGER
    );
    assert_eq!(lamports(&mut ctx, black.pubkey()).await, black_before);
    assert_eq!(lamports(&mut ctx, escrow_pda(id)).await, 0);
}

#[tokio::test]
async fn duplicate_cancellation_is_safe_and_pays_nothing_twice() {
    let id = 70_003;
    let Table { mut ctx, white, .. } = waiting_table(id).await;
    let cancel = cancel_ix(id, white.pubkey(), white.pubkey(), Pubkey::default());
    send(&mut ctx, cancel.clone(), &[&white]).await.unwrap();
    let after_first = lamports(&mut ctx, white.pubkey()).await;

    // A retry after a lost acknowledgement.
    next_blockhash(&mut ctx).await;
    send(&mut ctx, cancel, &[&white])
        .await
        .expect("repeat cancellation is accepted (idempotent)");
    assert_eq!(lamports(&mut ctx, white.pubkey()).await, after_first);
    assert_eq!(lamports(&mut ctx, escrow_pda(id)).await, 0);
    assert_eq!(fetch_game(&mut ctx, id).await.status, GameStatus::Cancelled);
}

#[tokio::test]
async fn outsider_cannot_cancel_an_abandoned_active_game_but_a_player_can() {
    let id = 70_004;
    let white = Keypair::new();
    let black = Keypair::new();
    let outsider = Keypair::new();
    let mut ctx = start(vec![
        (white.pubkey(), system_account(PLAYER_FUNDS)),
        (black.pubkey(), system_account(PLAYER_FUNDS)),
        (outsider.pubkey(), system_account(PLAYER_FUNDS)),
        (escrow_pda(id), system_account(2 * WAGER)),
    ])
    .await;
    let t = now(&mut ctx).await;
    let payer = ctx.payer.pubkey();
    // Moves played, idle for over the 24h abandonment window.
    let (key, data) = game(
        id,
        white.pubkey(),
        black.pubkey(),
        payer,
        GameStatus::Active,
        6,
        t - 3600 * 25,
    );
    ctx.set_account(&key, &AccountSharedData::from(data));

    let err = send(
        &mut ctx,
        cancel_ix(id, outsider.pubkey(), white.pubkey(), black.pubkey()),
        &[&outsider],
    )
    .await
    .expect_err("a non-player cannot cancel");
    assert_eq!(custom_code(&err), Some(ec(GameErrorCode::NotInGame)));
    assert_eq!(fetch_game(&mut ctx, id).await.status, GameStatus::Active);
    assert_eq!(lamports(&mut ctx, escrow_pda(id)).await, 2 * WAGER);

    send(
        &mut ctx,
        cancel_ix(id, black.pubkey(), white.pubkey(), black.pubkey()),
        &[&black],
    )
    .await
    .expect("a player may cancel the abandoned game");
    assert_eq!(fetch_game(&mut ctx, id).await.status, GameStatus::Cancelled);
    assert_eq!(lamports(&mut ctx, escrow_pda(id)).await, 0);
}

#[tokio::test]
async fn abandoned_cancel_is_refused_before_the_window() {
    let id = 70_005;
    let white = Keypair::new();
    let black = Keypair::new();
    let mut ctx = start(vec![
        (white.pubkey(), system_account(PLAYER_FUNDS)),
        (black.pubkey(), system_account(PLAYER_FUNDS)),
        (escrow_pda(id), system_account(2 * WAGER)),
    ])
    .await;
    let t = now(&mut ctx).await;
    let payer = ctx.payer.pubkey();
    let (key, data) = game(
        id,
        white.pubkey(),
        black.pubkey(),
        payer,
        GameStatus::Active,
        6,
        t - 600,
    );
    ctx.set_account(&key, &AccountSharedData::from(data));

    // A disconnected-but-not-abandoned game cannot be cancelled out from
    // under the opponent: network loss alone never ends it.
    let err = send(
        &mut ctx,
        cancel_ix(id, white.pubkey(), white.pubkey(), black.pubkey()),
        &[&white],
    )
    .await
    .expect_err("cancellation before the abandonment window must fail");
    assert_eq!(custom_code(&err), Some(ec(GameErrorCode::GameNotExpired)));
    assert_eq!(fetch_game(&mut ctx, id).await.status, GameStatus::Active);
}

/// Active, timed game: black to move (`turn` even) and idle past the window.
async fn idle_active_table(game_id: u64) -> Table {
    let white = Keypair::new();
    let black = Keypair::new();
    let mut ctx = start(vec![]).await;
    let t = now(&mut ctx).await;
    let (pda, bump) = game_pda(game_id);
    let g = Game {
        game_id,
        white: white.pubkey(),
        black: black.pubkey(),
        status: GameStatus::Active,
        last_move_timestamp: t - 200,
        fees_advanced: 0,
        fee_payer: white.pubkey(),
        result: GameResult::None,
        board_state: start_board(),
        move_count: 1,
        halfmove_clock: 0,
        turn: 2,
        created_at: t - 300,
        updated_at: t - 200,
        wager_amount: 0,
        wager_token: None,
        game_type: GameType::PvP,
        match_type: MatchType::Free,
        country_fee: 0,
        base_time_seconds: 300,
        increment_seconds: 0,
        bump,
        is_delegated: false,
        tournament_id: None,
        nonce: 1,
        draw_offered_by: None,
    };
    ctx.set_account(
        &pda,
        &AccountSharedData::from(program_account(&g, 8 + Game::INIT_SPACE)),
    );
    Table { ctx, white, black }
}

#[tokio::test]
async fn resignation_then_timeout_claim_keeps_the_single_result() {
    let id = 70_006;
    let Table {
        mut ctx,
        white,
        black,
    } = idle_active_table(id).await;

    send(&mut ctx, resign_ix(id, white.pubkey()), &[&white])
        .await
        .expect("white resigns");
    let g = fetch_game(&mut ctx, id).await;
    assert_eq!(g.status, GameStatus::Finished);
    assert_eq!(g.result, GameResult::Winner(black.pubkey()));

    // Black's clock had also lapsed; the late timeout claim must not flip it.
    let payer = ctx.payer.pubkey();
    let err = send(&mut ctx, claim_timeout_ix(id, payer), &[])
        .await
        .expect_err("timeout after a terminal result must fail");
    assert_eq!(custom_code(&err), Some(ec(GameErrorCode::GameNotActive)));
    assert_eq!(
        fetch_game(&mut ctx, id).await.result,
        GameResult::Winner(black.pubkey())
    );
}

#[tokio::test]
async fn timeout_then_resignation_keeps_the_single_result() {
    let id = 70_007;
    let Table {
        mut ctx,
        white,
        black,
    } = idle_active_table(id).await;

    let payer = ctx.payer.pubkey();
    send(&mut ctx, claim_timeout_ix(id, payer), &[])
        .await
        .expect("black flagged");
    assert_eq!(
        fetch_game(&mut ctx, id).await.result,
        GameResult::Winner(white.pubkey())
    );

    // The flagged player resigns late (e.g. reconnecting): no second result.
    assert!(send(&mut ctx, resign_ix(id, black.pubkey()), &[&black])
        .await
        .is_err());
    let g = fetch_game(&mut ctx, id).await;
    assert_eq!(g.status, GameStatus::Finished);
    assert_eq!(g.result, GameResult::Winner(white.pubkey()));
}

#[tokio::test]
async fn cancelled_game_cannot_be_resigned_or_timed_out() {
    let id = 70_008;
    let Table {
        mut ctx,
        white,
        black,
    } = waiting_table(id).await;
    send(
        &mut ctx,
        cancel_ix(id, white.pubkey(), white.pubkey(), Pubkey::default()),
        &[&white],
    )
    .await
    .unwrap();

    assert!(send(&mut ctx, resign_ix(id, white.pubkey()), &[&white])
        .await
        .is_err());
    let payer = ctx.payer.pubkey();
    assert!(send(&mut ctx, claim_timeout_ix(id, payer), &[])
        .await
        .is_err());
    let _ = black;
    let g = fetch_game(&mut ctx, id).await;
    assert_eq!(g.status, GameStatus::Cancelled);
    assert_eq!(g.result, GameResult::None);
}
