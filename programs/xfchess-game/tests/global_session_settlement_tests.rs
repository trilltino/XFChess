//! Regression coverage for a revenue-integrity bug found while auditing the
//! signing/popup architecture: the client's `global_create_game` path used to
//! hardcode `platform_fee_lamports = 0` (see `src/multiplayer/solana/lobby.rs`),
//! so every wagered game created via the "one popup ever" path paid zero
//! platform rake even though the on-chain settlement math (below) has always
//! correctly paid `Game.country_fee` to the treasury vault once it's actually
//! set to something nonzero at creation. This file exercises that on-chain
//! side directly: seed a `Finished` game with a nonzero `country_fee` and
//! confirm `finalize_game` pays exactly that amount to `treasury_vault`.
//!
//! Prereq: `cargo build-sbf` (see docs/ER_TESTING.md).

mod common;

use anchor_lang::{InstructionData, Space, ToAccountMetas};
use common::*;
use solana_sdk::{instruction::Instruction, pubkey::Pubkey, signature::Keypair, signer::Signer};
use xfchess_game::errors::GameErrorCode;
use xfchess_game::state::{Game, GameResult, GameStatus, GameType, MatchType, PlayerProfile};

fn treasury_vault_pda() -> Pubkey {
    Pubkey::find_program_address(&[b"treasury_vault"], &xfchess_game::ID).0
}

fn profile_pda(player: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"profile", player.as_ref()], &xfchess_game::ID).0
}

fn profile_account(authority: Pubkey) -> (Pubkey, solana_sdk::account::Account) {
    let pda = profile_pda(&authority);
    let profile = PlayerProfile {
        authority,
        ..Default::default()
    };
    (
        pda,
        program_account(&profile, 8 + PlayerProfile::INIT_SPACE),
    )
}

/// A `Finished`, undelegated, ranked (non-Free) game with a winner already
/// decided and a nonzero `country_fee` — exactly the state `finalize_game`
/// expects once `settlement_worker` (or, on the ER path, undelegation) has
/// moved a completed game back to the base layer.
#[allow(clippy::too_many_arguments)]
fn finished_game_account(
    game_id: u64,
    white: Pubkey,
    black: Pubkey,
    fee_payer: Pubkey,
    wager_amount: u64,
    country_fee: u64,
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
        result: GameResult::Winner(white),
        board_state: start_board(),
        move_count: 10,
        halfmove_clock: 0,
        turn: 11,
        created_at: 0,
        updated_at: 0,
        wager_amount,
        wager_token: None,
        game_type: GameType::PvP,
        match_type: MatchType::Rated,
        country_fee,
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

fn finalize_game_ix(game_id: u64, white: Pubkey, black: Pubkey, fee_payer: Pubkey) -> Instruction {
    let accounts = xfchess_game::__client_accounts_end_game::EndGame {
        game: game_pda(game_id).0,
        white_profile: profile_pda(&white),
        black_profile: profile_pda(&black),
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

fn escrow_pda(game_id: u64) -> Pubkey {
    Pubkey::find_program_address(&[b"escrow", &game_id.to_le_bytes()], &xfchess_game::ID).0
}

#[tokio::test]
async fn finalize_game_pays_the_nonzero_country_fee_to_treasury() {
    let white = Pubkey::new_unique();
    let black = Pubkey::new_unique();
    let fee_payer = Keypair::new();
    let game_id = 4045u64; // 0.00045 SOL below — the exact figure the popup-fee bug surfaced as
    let wager_amount = 10_000_000u64; // 0.01 SOL
    let country_fee = 450_000u64; // 0.00045 SOL — the platform fee this test guards

    let (game_pda_key, game_account_data) = finished_game_account(
        game_id,
        white,
        black,
        fee_payer.pubkey(),
        wager_amount,
        country_fee,
    );
    let (white_profile_pda, white_profile_data) = profile_account(white);
    let (black_profile_pda, black_profile_data) = profile_account(black);

    let mut ctx = start(vec![
        (game_pda_key, game_account_data),
        (white_profile_pda, white_profile_data),
        (black_profile_pda, black_profile_data),
        (escrow_pda(game_id), system_account(wager_amount * 2)),
        (treasury_vault_pda(), system_account(1_000_000)),
        (fee_payer.pubkey(), system_account(1_000_000)),
        (white, system_account(1_000_000)),
        (black, system_account(1_000_000)),
    ])
    .await;

    let treasury_before = ctx
        .banks_client
        .get_balance(treasury_vault_pda())
        .await
        .unwrap();

    let ix = finalize_game_ix(game_id, white, black, fee_payer.pubkey());
    send(&mut ctx, ix, &[])
        .await
        .expect("finalize_game should settle a Finished game with a nonzero country_fee");

    let treasury_after = ctx
        .banks_client
        .get_balance(treasury_vault_pda())
        .await
        .unwrap();

    // fees_advanced is 0 in this fixture, so the vault's only inflow is the
    // country_fee itself — isolates exactly the value this regression test
    // is guarding (see module doc).
    assert_eq!(
        treasury_after - treasury_before,
        country_fee,
        "treasury_vault must receive exactly the game's country_fee at settlement"
    );

    // Game account closes to fee_payer on finalize — confirms settlement
    // actually ran to completion rather than erroring out before reaching it.
    let closed = ctx.banks_client.get_account(game_pda_key).await.unwrap();
    assert!(
        closed.is_none(),
        "Game account should be closed after finalize_game"
    );
}

/// docs/PRE_MAINNET_E2E_PLAN.md §1.1: `EndGame`'s `fee_payer` constraint
/// (`finalize.rs:39`, `constraint = fee_payer.key() == game.fee_payer`) is a
/// repo-wide invariant enforced at four sites, but before this test nothing
/// in the test suite ever supplied a *mismatched* fee_payer — only the
/// correct one. This exercises the negative case directly.
#[tokio::test]
async fn finalize_game_rejects_mismatched_fee_payer() {
    let white = Pubkey::new_unique();
    let black = Pubkey::new_unique();
    let real_fee_payer = Keypair::new();
    let wrong_fee_payer = Keypair::new(); // funded, but NOT game.fee_payer
    let game_id = 4046u64;
    let wager_amount = 10_000_000u64;
    let country_fee = 450_000u64;

    let (game_pda_key, game_account_data) = finished_game_account(
        game_id,
        white,
        black,
        real_fee_payer.pubkey(), // game.fee_payer records the REAL payer
        wager_amount,
        country_fee,
    );
    let (white_profile_pda, white_profile_data) = profile_account(white);
    let (black_profile_pda, black_profile_data) = profile_account(black);

    let mut ctx = start(vec![
        (game_pda_key, game_account_data),
        (white_profile_pda, white_profile_data),
        (black_profile_pda, black_profile_data),
        (escrow_pda(game_id), system_account(wager_amount * 2)),
        (treasury_vault_pda(), system_account(1_000_000)),
        (real_fee_payer.pubkey(), system_account(1_000_000)),
        (wrong_fee_payer.pubkey(), system_account(1_000_000)),
        (white, system_account(1_000_000)),
        (black, system_account(1_000_000)),
    ])
    .await;

    // Substitute the wrong (but validly funded) SystemAccount for `fee_payer`.
    let ix = finalize_game_ix(game_id, white, black, wrong_fee_payer.pubkey());
    let err = send(&mut ctx, ix, &[])
        .await
        .expect_err("finalize_game must reject a fee_payer that doesn't match game.fee_payer");

    assert_eq!(
        custom_code(&err),
        Some(ec(GameErrorCode::FeePayerMismatch)),
        "expected FeePayerMismatch, got {err:?}"
    );

    // The rejected transaction must not have mutated or closed the game.
    let game = fetch_game(&mut ctx, game_id).await;
    assert_eq!(game.status, GameStatus::Finished);
}

/// docs/PRE_MAINNET_E2E_PLAN.md §1.1: regression guard on the *correct*-payer
/// path — asserts `fee_payer`'s balance delta is exactly the escrow tx-fee
/// reimbursement plus the closed `Game` account's full rent, so a future
/// change to `close =`'s target (or to the escrow tx-fee split) that quietly
/// shortchanges the relayer fails this test instead of shipping unnoticed.
#[tokio::test]
async fn finalize_game_refunds_exact_rent_and_tx_fee_to_correct_fee_payer() {
    let white = Pubkey::new_unique();
    let black = Pubkey::new_unique();
    let fee_payer = Keypair::new();
    let game_id = 4047u64;
    let wager_amount = 10_000_000u64; // pot = 20_000_000
    let country_fee = 450_000u64;
    let fee_payer_starting_balance = 1_000_000u64;

    let (game_pda_key, game_account_data) = finished_game_account(
        game_id,
        white,
        black,
        fee_payer.pubkey(),
        wager_amount,
        country_fee,
    );
    // `common::program_account` allocates every program account with a fixed
    // 1_000_000_000-lamport balance — that full amount is what `close =
    // fee_payer` refunds as rent once the Game account is closed.
    let game_account_rent = game_account_data.lamports;

    let (white_profile_pda, white_profile_data) = profile_account(white);
    let (black_profile_pda, black_profile_data) = profile_account(black);

    let mut ctx = start(vec![
        (game_pda_key, game_account_data),
        (white_profile_pda, white_profile_data),
        (black_profile_pda, black_profile_data),
        (escrow_pda(game_id), system_account(wager_amount * 2)),
        (treasury_vault_pda(), system_account(1_000_000)),
        (fee_payer.pubkey(), system_account(fee_payer_starting_balance)),
        (white, system_account(1_000_000)),
        (black, system_account(1_000_000)),
    ])
    .await;

    let ix = finalize_game_ix(game_id, white, black, fee_payer.pubkey());
    send(&mut ctx, ix, &[])
        .await
        .expect("finalize_game should settle with the correct fee_payer");

    let fee_payer_after = ctx
        .banks_client
        .get_balance(fee_payer.pubkey())
        .await
        .unwrap();

    // escrow tx-fee reimbursement is min(10_000, pot) — the pot here (2 * 10M)
    // comfortably exceeds 10_000, so the reimbursement is the flat 10_000.
    let expected_tx_fee_reimbursement = 10_000u64;
    let expected_delta = game_account_rent + expected_tx_fee_reimbursement;

    assert_eq!(
        fee_payer_after,
        fee_payer_starting_balance + expected_delta,
        "fee_payer must receive exactly the closed Game account's rent plus the escrow tx-fee reimbursement"
    );
}

/// docs/PRE_MAINNET_E2E_PLAN.md §2.3: the P2P gossip layer's `SessionInfo`
/// binds `opponent_pubkey` from a self-asserted, unverified claim (see
/// `multiplayer::systems::handle_session_info_from_network` and its paired
/// unit test `session_info_spoof_tests::handle_session_info_accepts_a_spoofed_player_pubkey_with_no_verification`
/// in `src/multiplayer/systems.rs`), and that spoofed value can genuinely
/// reach `finalize_game`'s account list client-side
/// (`src/multiplayer/rollup/bridge.rs`'s finalize-on-end path derives
/// white/black wallet accounts from `opponent_pubkey`). This proves the
/// other half of the boundary: even when the client is fooled into building
/// `finalize_game` with a spoofed `black_authority`, the on-chain
/// `constraint = black_authority.key() == game.black` check rejects it and
/// no escrow funds move — the informal "money layer is separate from the
/// gossip layer" claim, now an explicit regression test.
fn finalize_game_ix_with_spoofed_black_authority(
    game_id: u64,
    white: Pubkey,
    real_black: Pubkey,
    spoofed_black_authority: Pubkey,
    fee_payer: Pubkey,
) -> Instruction {
    let accounts = xfchess_game::__client_accounts_end_game::EndGame {
        game: game_pda(game_id).0,
        white_profile: profile_pda(&white),
        // Correct PDA (matches the real on-chain game.black) — isolates the
        // `black_authority` constraint from the separate `black_profile`
        // seeds constraint, which would otherwise fail first.
        black_profile: profile_pda(&real_black),
        white_authority: white,
        black_authority: spoofed_black_authority, // simulates a spoofed opponent_pubkey
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

#[tokio::test]
async fn finalize_game_rejects_spoofed_black_authority() {
    let white = Pubkey::new_unique();
    let real_black = Pubkey::new_unique();
    let spoofed_black_authority = Pubkey::new_unique(); // NOT game.black
    let fee_payer = Keypair::new();
    let game_id = 4048u64;
    let wager_amount = 10_000_000u64;
    let country_fee = 450_000u64;

    let (game_pda_key, game_account_data) = finished_game_account(
        game_id,
        white,
        real_black,
        fee_payer.pubkey(),
        wager_amount,
        country_fee,
    );
    let (white_profile_pda, white_profile_data) = profile_account(white);
    let (black_profile_pda, black_profile_data) = profile_account(real_black);

    let mut ctx = start(vec![
        (game_pda_key, game_account_data),
        (white_profile_pda, white_profile_data),
        (black_profile_pda, black_profile_data),
        (escrow_pda(game_id), system_account(wager_amount * 2)),
        (treasury_vault_pda(), system_account(1_000_000)),
        (fee_payer.pubkey(), system_account(1_000_000)),
        (white, system_account(1_000_000)),
        (real_black, system_account(1_000_000)),
        (spoofed_black_authority, system_account(1_000_000)),
    ])
    .await;

    let escrow_before = ctx
        .banks_client
        .get_balance(escrow_pda(game_id))
        .await
        .unwrap();
    let spoofed_balance_before = ctx
        .banks_client
        .get_balance(spoofed_black_authority)
        .await
        .unwrap();

    let ix = finalize_game_ix_with_spoofed_black_authority(
        game_id,
        white,
        real_black,
        spoofed_black_authority,
        fee_payer.pubkey(),
    );
    let err = send(&mut ctx, ix, &[])
        .await
        .expect_err("finalize_game must reject a black_authority that doesn't match game.black");

    assert_eq!(
        custom_code(&err),
        Some(ec(GameErrorCode::UnauthorizedAccess)),
        "expected UnauthorizedAccess, got {err:?}"
    );

    // No funds moved anywhere, and the game was not closed.
    let escrow_after = ctx
        .banks_client
        .get_balance(escrow_pda(game_id))
        .await
        .unwrap();
    let spoofed_balance_after = ctx
        .banks_client
        .get_balance(spoofed_black_authority)
        .await
        .unwrap();
    assert_eq!(escrow_before, escrow_after, "escrow must be untouched");
    assert_eq!(
        spoofed_balance_before, spoofed_balance_after,
        "the spoofed account must receive nothing"
    );
    let game = fetch_game(&mut ctx, game_id).await;
    assert_eq!(
        game.status,
        GameStatus::Finished,
        "a rejected finalize must not close or mutate the game"
    );
}
