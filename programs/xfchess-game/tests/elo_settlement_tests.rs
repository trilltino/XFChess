//! Proves `finalize_game` actually mutates `PlayerProfile.elo_rating` (and the
//! surrounding win/loss/streak bookkeeping) correctly when run through the
//! real compiled program — not just that the pure K=32 formula in
//! `elo::glicko2::calculate_elo_update` is right in isolation.
//!
//! Before this file: `game_settlement_tests.rs` and
//! `global_session_settlement_tests.rs` already exercise `finalize_game`
//! through the same `ProgramTest` harness, but only assert on escrow
//! payout/fee-payer behavior — neither ever reads back `elo_rating`, `wins`,
//! `losses`, `win_streak`, or `ranked_games`. `calculate_elo_update`'s own
//! unit tests prove the math; nothing before this proved the instruction
//! actually applies that math to the right accounts.
//!
//! Prereq: `cargo build-sbf` (see docs/ER_TESTING.md).

mod common;

use anchor_lang::{InstructionData, Space, ToAccountMetas};
use common::*;
use solana_program_test::ProgramTestBanksClientExt;
use solana_sdk::{instruction::Instruction, pubkey::Pubkey, signature::Keypair, signer::Signer};
use xfchess_game::elo::glicko2::calculate_elo_update;
use xfchess_game::state::{Game, GameResult, GameStatus, GameType, MatchType, PlayerProfile};

fn treasury_vault_pda() -> Pubkey {
    Pubkey::find_program_address(&[b"treasury_vault"], &xfchess_game::ID).0
}

/// A profile seeded at a specific rating, rather than the zeroed
/// `Default::default()` other settlement tests use — needed so the ELO
/// change is actually observable and comparable against
/// `calculate_elo_update`'s own output.
fn rated_profile_account(
    authority: Pubkey,
    elo_rating: f64,
) -> (Pubkey, solana_sdk::account::Account) {
    let pda = profile_pda(&authority).0;
    let profile = PlayerProfile {
        authority,
        elo_rating,
        ..Default::default()
    };
    (
        pda,
        program_account(&profile, 8 + PlayerProfile::INIT_SPACE),
    )
}

#[allow(clippy::too_many_arguments)]
fn settlement_game_account(
    game_id: u64,
    white: Pubkey,
    black: Pubkey,
    fee_payer: Pubkey,
    result: GameResult,
    match_type: MatchType,
) -> (Pubkey, solana_sdk::account::Account) {
    let (pda, bump) = game_pda(game_id);
    let game = Game {
        game_id,
        white,
        black,
        status: GameStatus::Finished,
        last_move_timestamp: 0,
        fees_advanced: 0,
        fee_payer,
        result,
        board_state: start_board(),
        move_count: 10,
        halfmove_clock: 0,
        turn: 11,
        created_at: 0,
        updated_at: 0,
        wager_amount: 0,
        wager_token: None,
        game_type: GameType::PvP,
        match_type,
        country_fee: 0,
        base_time_seconds: 300,
        increment_seconds: 0,
        bump,
        is_delegated: false,
        tournament_id: None,
        nonce: 0,
        draw_offered_by: None,
    };
    (pda, program_account(&game, 8 + Game::INIT_SPACE))
}

/// `common::send` reuses `ctx.last_blockhash` as-is; an identical instruction
/// sent again on the same blockhash produces a byte-identical transaction
/// signature, which `banks_client` silently dedupes as "already processed"
/// instead of re-executing the handler — so a naive replay test would pass
/// for the wrong reason (never actually hitting the closed account). Refresh
/// the blockhash first so the replay is a genuinely distinct transaction.
async fn send_as_distinct_tx(
    ctx: &mut solana_program_test::ProgramTestContext,
    ix: Instruction,
    extra: &[&Keypair],
) -> Result<(), solana_sdk::transaction::TransactionError> {
    let blockhash = ctx
        .banks_client
        .get_new_latest_blockhash(&ctx.last_blockhash)
        .await
        .expect("failed to get a fresh blockhash");
    ctx.last_blockhash = blockhash;
    send(ctx, ix, extra).await
}

fn escrow_pda(game_id: u64) -> Pubkey {
    Pubkey::find_program_address(&[b"escrow", &game_id.to_le_bytes()], &xfchess_game::ID).0
}

fn finalize_game_ix(game_id: u64, white: Pubkey, black: Pubkey, fee_payer: Pubkey) -> Instruction {
    let accounts = xfchess_game::__client_accounts_end_game::EndGame {
        game: game_pda(game_id).0,
        white_profile: profile_pda(&white).0,
        black_profile: profile_pda(&black).0,
        white_authority: white,
        black_authority: black,
        escrow_pda: escrow_pda(game_id),
        treasury_vault: treasury_vault_pda(),
        fee_payer,
        system_program: solana_system_interface::program::ID,
    }
    .to_account_metas(None);
    let data = xfchess_game::instruction::FinalizeGame { game_id }.data();
    Instruction {
        program_id: xfchess_game::ID,
        accounts,
        data,
    }
}

/// `finalize_game`'s standard funded fixture: game + both profiles + escrow
/// (zero wager, since these tests are isolating the ELO/stats side, not the
/// payout math already covered elsewhere) + treasury/fee-payer/authority
/// system accounts.
#[allow(clippy::too_many_arguments)]
async fn start_with_profiles(
    game_id: u64,
    white: Pubkey,
    black: Pubkey,
    fee_payer: Pubkey,
    white_elo: f64,
    black_elo: f64,
    result: GameResult,
    match_type: MatchType,
) -> solana_program_test::ProgramTestContext {
    let (game_pda_key, game_account_data) =
        settlement_game_account(game_id, white, black, fee_payer, result, match_type);
    let (white_profile_pda, white_profile_data) = rated_profile_account(white, white_elo);
    let (black_profile_pda, black_profile_data) = rated_profile_account(black, black_elo);

    start(vec![
        (game_pda_key, game_account_data),
        (white_profile_pda, white_profile_data),
        (black_profile_pda, black_profile_data),
        (escrow_pda(game_id), system_account(0)),
        (treasury_vault_pda(), system_account(1_000_000)),
        (fee_payer, system_account(1_000_000)),
        (white, system_account(1_000_000)),
        (black, system_account(1_000_000)),
    ])
    .await
}

#[tokio::test]
async fn finalize_game_applies_k32_elo_update_to_winner_and_loser() {
    let white = Pubkey::new_unique();
    let black = Pubkey::new_unique();
    let fee_payer = Keypair::new();
    let game_id = 9101u64;
    let white_elo = 120_000.0; // 1200
    let black_elo = 120_000.0; // 1200

    let mut ctx = start_with_profiles(
        game_id,
        white,
        black,
        fee_payer.pubkey(),
        white_elo,
        black_elo,
        GameResult::Winner(white),
        MatchType::Rated,
    )
    .await;

    let ix = finalize_game_ix(game_id, white, black, fee_payer.pubkey());
    send(&mut ctx, ix, &[])
        .await
        .expect("finalize_game should settle a Finished Rated game");

    let (expected_white, expected_black) = calculate_elo_update(white_elo, black_elo, 1.0);

    let white_profile = fetch_profile(&mut ctx, &white).await;
    let black_profile = fetch_profile(&mut ctx, &black).await;

    assert!(
        (white_profile.elo_rating - expected_white).abs() < 1.0,
        "winner's rating must match calculate_elo_update's own output: got {}, expected {}",
        white_profile.elo_rating,
        expected_white
    );
    assert!(
        (black_profile.elo_rating - expected_black).abs() < 1.0,
        "loser's rating must match calculate_elo_update's own output: got {}, expected {}",
        black_profile.elo_rating,
        expected_black
    );
    assert!(
        white_profile.elo_rating > white_elo,
        "winner's rating must have gone up"
    );
    assert!(
        black_profile.elo_rating < black_elo,
        "loser's rating must have gone down"
    );

    assert_eq!(white_profile.wins, 1);
    assert_eq!(white_profile.losses, 0);
    assert_eq!(black_profile.wins, 0);
    assert_eq!(black_profile.losses, 1);
    assert_eq!(white_profile.win_streak, 1);
    assert_eq!(white_profile.best_streak, 1);
    assert_eq!(black_profile.win_streak, 0);
    assert_eq!(white_profile.games_played, 1);
    assert_eq!(black_profile.games_played, 1);
    assert_eq!(white_profile.ranked_games, 1);
    assert_eq!(black_profile.ranked_games, 1);
    assert!(white_profile.last_played > 0);
    assert!(black_profile.last_played > 0);
}

#[tokio::test]
async fn finalize_game_applies_elo_update_for_a_draw() {
    let white = Pubkey::new_unique();
    let black = Pubkey::new_unique();
    let fee_payer = Keypair::new();
    let game_id = 9102u64;
    // Unequal ratings so a draw's rating shift is actually observable
    // (higher-rated white should lose a little, lower-rated black gain).
    let white_elo = 140_000.0; // 1400
    let black_elo = 120_000.0; // 1200

    let mut ctx = start_with_profiles(
        game_id,
        white,
        black,
        fee_payer.pubkey(),
        white_elo,
        black_elo,
        GameResult::Draw,
        MatchType::Rated,
    )
    .await;

    let ix = finalize_game_ix(game_id, white, black, fee_payer.pubkey());
    send(&mut ctx, ix, &[])
        .await
        .expect("finalize_game should settle a Finished Rated draw");

    let (expected_white, expected_black) = calculate_elo_update(white_elo, black_elo, 0.5);

    let white_profile = fetch_profile(&mut ctx, &white).await;
    let black_profile = fetch_profile(&mut ctx, &black).await;

    assert!((white_profile.elo_rating - expected_white).abs() < 1.0);
    assert!((black_profile.elo_rating - expected_black).abs() < 1.0);
    assert!(
        white_profile.elo_rating < white_elo,
        "favorite drawing a lower-rated opponent must lose rating"
    );
    assert!(
        black_profile.elo_rating > black_elo,
        "underdog drawing a higher-rated opponent must gain rating"
    );

    assert_eq!(white_profile.draws, 1);
    assert_eq!(black_profile.draws, 1);
    assert_eq!(white_profile.wins, 0);
    assert_eq!(white_profile.losses, 0);
    assert_eq!(white_profile.win_streak, 0);
    assert_eq!(black_profile.win_streak, 0);
}

/// `settlement.rs`'s `if match_type != MatchType::Free` gate must keep
/// `elo_rating`/`ranked_games` untouched for a Free (unwagered) game, even
/// though `games_played`/`wins`/`losses` still increment for every game
/// regardless of match type.
#[tokio::test]
async fn finalize_game_does_not_change_elo_for_a_free_match() {
    let white = Pubkey::new_unique();
    let black = Pubkey::new_unique();
    let fee_payer = Keypair::new();
    let game_id = 9103u64;
    let white_elo = 120_000.0;
    let black_elo = 120_000.0;

    let mut ctx = start_with_profiles(
        game_id,
        white,
        black,
        fee_payer.pubkey(),
        white_elo,
        black_elo,
        GameResult::Winner(white),
        MatchType::Free,
    )
    .await;

    let ix = finalize_game_ix(game_id, white, black, fee_payer.pubkey());
    send(&mut ctx, ix, &[])
        .await
        .expect("finalize_game should settle a Finished Free game");

    let white_profile = fetch_profile(&mut ctx, &white).await;
    let black_profile = fetch_profile(&mut ctx, &black).await;

    assert_eq!(
        white_profile.elo_rating, white_elo,
        "a Free game must never move ELO"
    );
    assert_eq!(
        black_profile.elo_rating, black_elo,
        "a Free game must never move ELO"
    );
    assert_eq!(white_profile.ranked_games, 0, "Free games aren't ranked");
    assert_eq!(black_profile.ranked_games, 0, "Free games aren't ranked");

    // Win/loss/games_played bookkeeping still applies to Free games — only
    // the rating and ranked-games counter are gated.
    assert_eq!(white_profile.wins, 1);
    assert_eq!(black_profile.losses, 1);
    assert_eq!(white_profile.games_played, 1);
    assert_eq!(black_profile.games_played, 1);
}

/// The `Game` account has `close = fee_payer`, so a second `finalize_game`
/// for the same `game_id` can't execute at all — the account is gone. This
/// pins that as the mechanism that makes "one settled game -> exactly one
/// ELO mutation" hold, by proving the second call fails outright rather than
/// silently re-applying the K=32 update.
#[tokio::test]
async fn finalize_game_cannot_be_replayed_to_double_apply_elo() {
    let white = Pubkey::new_unique();
    let black = Pubkey::new_unique();
    let fee_payer = Keypair::new();
    let game_id = 9104u64;
    let white_elo = 120_000.0;
    let black_elo = 120_000.0;

    let mut ctx = start_with_profiles(
        game_id,
        white,
        black,
        fee_payer.pubkey(),
        white_elo,
        black_elo,
        GameResult::Winner(white),
        MatchType::Rated,
    )
    .await;

    let ix = finalize_game_ix(game_id, white, black, fee_payer.pubkey());
    send(&mut ctx, ix, &[]).await.expect("first finalize must succeed");

    let after_first = fetch_profile(&mut ctx, &white).await;
    assert!(after_first.elo_rating > white_elo);

    // The Game account is closed now — replaying the exact same instruction
    // must fail (account no longer exists to load), not re-run settlement.
    let replay = finalize_game_ix(game_id, white, black, fee_payer.pubkey());
    let err = send_as_distinct_tx(&mut ctx, replay, &[])
        .await
        .expect_err("a second finalize_game for the same game_id must fail");
    let _ = err; // exact error shape (AccountNotInitialized) isn't the point; failure is.

    let after_replay = fetch_profile(&mut ctx, &white).await;
    assert_eq!(
        after_replay.elo_rating, after_first.elo_rating,
        "a rejected replay must not mutate elo_rating a second time"
    );
}
