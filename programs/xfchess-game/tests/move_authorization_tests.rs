//! P1: only the authenticated session of the side to move can advance a game,
//! and every rejected move leaves the Game byte-for-byte unchanged.
#[path = "common/agave.rs"]
pub mod agave;
use anyhow::Context as _;
pub mod common;

use anchor_lang::AccountDeserialize;
use common::*;
use solana_sdk::account::Account;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};
use xfchess_game::errors::GameErrorCode;
use xfchess_game::state::{Game, GameResult, GameStatus};

const GAME_ID: u64 = 42;

fn edit_game(account: &Account, edit: impl FnOnce(&mut Game)) -> anyhow::Result<Account> {
    let mut game = Game::try_deserialize(&mut account.data.as_slice())?;
    edit(&mut game);
    program_account(&game, account.data.len())
}

async fn game_bytes(ctx: &mut agave::ProgramTestContext) -> anyhow::Result<Account> {
    ctx.banks_client
        .get_account(game_pda(GAME_ID).0)
        .await?
        .context("game missing")
}

#[tokio::test]
async fn a_rejected_move_never_mutates_the_game() -> anyhow::Result<()> {
    let white = Pubkey::new_unique();
    let black = Pubkey::new_unique();
    let board = start_board();
    let mv = uci("e2e4");
    let next = apply(&board, &mv)?;

    let stranger = Keypair::new();
    let registered = Keypair::new();
    // (label, signer is the registered key, session enabled, session game id,
    //  session owner is black, expected anchor/program error code)
    let cases: [(&str, bool, bool, u64, bool, u32); 4] = [
        (
            "unregistered signer",
            false,
            true,
            GAME_ID,
            false,
            ec(GameErrorCode::InvalidSessionKey),
        ),
        (
            "disabled session",
            true,
            false,
            GAME_ID,
            false,
            ec(GameErrorCode::SessionExpiredOrDisabled),
        ),
        (
            "session of another game",
            true,
            true,
            GAME_ID + 1,
            false,
            anchor_lang::error::ErrorCode::ConstraintSeeds as u32,
        ),
        (
            "blacks session on whites turn",
            true,
            true,
            GAME_ID,
            true,
            ec(GameErrorCode::NotYourTurn),
        ),
    ];
    for (label, uses_registered, enabled, session_game, black_owner, code) in cases {
        let signer = if uses_registered {
            &registered
        } else {
            &stranger
        };
        let owner = if black_owner { black } else { white };
        let (game_key, game_acc) =
            game_account(GAME_ID, white, black, board, 1, 0, GameStatus::Active)?;
        let mut ctx = start(vec![
            (game_key, game_acc.clone()),
            session_account(session_game, owner, registered.pubkey(), i64::MAX, enabled)?,
        ])
        .await;
        let mut ix = record_move_ix(GAME_ID, &owner, &signer.pubkey(), mv, next, 1, Some(0));
        if session_game != GAME_ID {
            ix.accounts[2].pubkey = sd_pda(session_game, &owner).0;
        }
        let err = send(&mut ctx, ix, &[signer])
            .await
            .err()
            .with_context(|| format!("{label} must fail"))?;
        assert_eq!(custom_code(&err), Some(code), "{label}");
        assert_eq!(
            game_bytes(&mut ctx).await?,
            game_acc,
            "{label}: game changed"
        );
    }
    Ok(())
}

#[tokio::test]
async fn a_legal_move_with_a_forged_next_board_is_rejected() -> anyhow::Result<()> {
    let session = Keypair::new();
    let (white, black) = (Pubkey::new_unique(), Pubkey::new_unique());
    let board = start_board();
    let (game_key, game_acc) =
        game_account(GAME_ID, white, black, board, 1, 0, GameStatus::Active)?;
    let mut ctx = start(vec![
        (game_key, game_acc.clone()),
        session_account(GAME_ID, white, session.pubkey(), i64::MAX, true)?,
    ])
    .await;
    // e2e4 is legal, but the submitted board is the result of e2e3.
    let forged = apply(&board, &uci("e2e3"))?;
    let ix = record_move_ix(
        GAME_ID,
        &white,
        &session.pubkey(),
        uci("e2e4"),
        forged,
        1,
        Some(0),
    );
    let err = send(&mut ctx, ix, &[&session])
        .await
        .err()
        .context("forged board must fail")?;
    assert_eq!(
        custom_code(&err),
        Some(ec(GameErrorCode::InvalidBoardState))
    );
    assert_eq!(game_bytes(&mut ctx).await?, game_acc);
    Ok(())
}

#[tokio::test]
async fn no_move_is_accepted_after_the_game_ends_or_at_the_nonce_ceiling() -> anyhow::Result<()> {
    let session = Keypair::new();
    let (white, black) = (Pubkey::new_unique(), Pubkey::new_unique());
    let board = start_board();
    let mv = uci("e2e4");
    let next = apply(&board, &mv)?;
    for (label, status, nonce, expected) in [
        (
            "finished",
            GameStatus::Finished,
            0u64,
            GameErrorCode::GameNotActive,
        ),
        (
            "settled",
            GameStatus::Settled,
            0,
            GameErrorCode::GameNotActive,
        ),
        (
            "nonce ceiling",
            GameStatus::Active,
            u64::MAX,
            GameErrorCode::ArithmeticOverflow,
        ),
    ] {
        let (game_key, game_acc) = game_account(GAME_ID, white, black, board, 1, nonce, status)?;
        let mut ctx = start(vec![
            (game_key, game_acc.clone()),
            session_account(GAME_ID, white, session.pubkey(), i64::MAX, true)?,
        ])
        .await;
        let ix = record_move_ix(GAME_ID, &white, &session.pubkey(), mv, next, 1, None);
        let err = send(&mut ctx, ix, &[&session])
            .await
            .err()
            .with_context(|| format!("{label} must fail"))?;
        assert_eq!(custom_code(&err), Some(ec(expected)), "{label}");
        assert_eq!(game_bytes(&mut ctx).await?, game_acc, "{label}");
    }
    Ok(())
}

#[tokio::test]
async fn stalemate_and_the_fifty_move_rule_end_the_game_as_a_draw() -> anyhow::Result<()> {
    let session = Keypair::new();
    let (white, black) = (Pubkey::new_unique(), Pubkey::new_unique());

    // Qc1-c7 leaves the black king on a8 with no legal move and no check.
    let stalemate = board_from_fen("k7/8/1K6/8/8/8/8/2Q5 w - - 0 1");
    // A quiet knight move with the clock already at 99 reaches 100 half-moves.
    let opening = start_board();
    for (label, board, mv, halfmove) in [
        ("stalemate", stalemate, "c1c7", 0u16),
        ("fifty-move", opening, "g1f3", 99),
    ] {
        let (game_key, base) =
            game_account(GAME_ID, white, black, board, 1, 0, GameStatus::Active)?;
        let game_acc = edit_game(&base, |g| g.halfmove_clock = halfmove)?;
        let mut ctx = start(vec![
            (game_key, game_acc),
            session_account(GAME_ID, white, session.pubkey(), i64::MAX, true)?,
        ])
        .await;
        let mv = uci(mv);
        let next = apply(&board, &mv)?;
        let ix = record_move_ix(GAME_ID, &white, &session.pubkey(), mv, next, 1, Some(0));
        send(&mut ctx, ix, &[&session])
            .await
            .with_context(|| format!("{label}: the move is legal"))?;
        let game = fetch_game(&mut ctx, GAME_ID).await?;
        assert_eq!(game.status, GameStatus::Finished, "{label}");
        assert_eq!(game.result, GameResult::Draw, "{label}");
    }
    Ok(())
}
