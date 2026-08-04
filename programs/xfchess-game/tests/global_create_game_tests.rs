//! `global_create_game`: the session-signed create path pays rent for the new
//! `Game` PDA and the wager escrow out of the `GlobalSessionDelegation` vault
//! — a PDA, not a wallet. This is instruction-level (`ProgramTest`) coverage
//! because the bug it guards against ("Cross-program invocation with
//! unauthorized signer or writable account: ... signer privilege escalated")
//! only shows up under the real BPF loader's CPI privilege checks: Anchor's
//! `init, payer = X` constraint CPIs into `system_program::create_account`
//! without ever supplying seeds for the payer side, so it silently compiles
//! fine but fails on-chain the moment `X` is a PDA instead of a wallet
//! `Signer`. A unit test constructing the handler's `Game` value directly
//! would never exercise that CPI at all.
//!
//! Prereq: `cargo build-sbf` (see docs/ER_TESTING.md).

mod common;

use anchor_lang::{AccountDeserialize, InstructionData, Space, ToAccountMetas};
use common::*;
use solana_sdk::{instruction::Instruction, pubkey::Pubkey, signature::Keypair, signer::Signer};
use xfchess_game::state::{
    Game, GameResult, GameStatus, GameType, GlobalSessionDelegation, MatchType, PlayerProfile,
};

fn global_session_pda(player: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[GlobalSessionDelegation::SEED, player.as_ref()],
        &xfchess_game::ID,
    )
}

fn escrow_pda(game_id: u64) -> Pubkey {
    Pubkey::find_program_address(&[b"escrow", &game_id.to_le_bytes()], &xfchess_game::ID).0
}

/// Seed a `GlobalSessionDelegation` with plenty of budget/games left and a
/// far-future expiry, standing in for a real prior `authorize_global_session`.
fn global_session_account(
    player: Pubkey,
    session_key: Pubkey,
    spending_limit: u64,
    max_wager: u64,
) -> (Pubkey, solana_sdk::account::Account) {
    let (pda, bump) = global_session_pda(&player);
    let session = GlobalSessionDelegation {
        player,
        session_key,
        expires_at: i64::MAX / 2,
        spending_limit,
        total_spent: 0,
        max_wager,
        games_remaining: 200,
        enabled: true,
        bump,
    };
    (
        pda,
        program_account(&session, 8 + GlobalSessionDelegation::INIT_SPACE),
    )
}

#[allow(clippy::too_many_arguments)]
fn global_create_game_ix(
    session_pda: Pubkey,
    session_signer: Pubkey,
    player: Pubkey,
    game_id: u64,
    wager_amount: u64,
    match_type: MatchType,
    platform_fee: u64,
    base_time_seconds: u64,
    increment_seconds: u16,
) -> Instruction {
    let accounts = xfchess_game::__client_accounts_global_create_game::GlobalCreateGame {
        session_delegation: session_pda,
        session_signer,
        player,
        game: game_pda(game_id).0,
        escrow_pda: escrow_pda(game_id),
        system_program: solana_system_interface::program::ID,
    }
    .to_account_metas(None);
    let data = xfchess_game::instruction::GlobalCreateGame {
        game_id,
        wager_amount,
        match_type,
        platform_fee,
        base_time_seconds,
        increment_seconds,
    }
    .data();
    Instruction {
        program_id: xfchess_game::ID,
        accounts,
        data,
    }
}

#[tokio::test]
async fn global_create_game_funds_rent_and_wager_from_session_vault() {
    let player = Pubkey::new_unique();
    let session_signer = Keypair::new();
    let wager_amount = 10_000_000u64; // 0.01 SOL — above MIN_WAGER_LAMPORTS

    let (session_pda, session_account_data) = global_session_account(
        player,
        session_signer.pubkey(),
        5_000_000_000,
        1_000_000_000,
    );

    let mut ctx = start(vec![(session_pda, session_account_data)]).await;

    let game_id = 42u64;
    let ix = global_create_game_ix(
        session_pda,
        session_signer.pubkey(),
        player,
        game_id,
        wager_amount,
        MatchType::Rated,
        0,
        300,
        0,
    );

    send(&mut ctx, ix, &[&session_signer])
        .await
        .expect("global_create_game should succeed when the session vault PDA funds rent + wager");

    let game = fetch_game(&mut ctx, game_id).await;
    assert_eq!(game.game_id, game_id);
    assert_eq!(game.white, player);
    assert_eq!(game.black, Pubkey::default());
    assert_eq!(game.status, GameStatus::WaitingForOpponent);
    assert_eq!(game.wager_amount, wager_amount);
    assert_eq!(game.fee_payer, session_signer.pubkey());

    let escrow_balance = ctx
        .banks_client
        .get_balance(escrow_pda(game_id))
        .await
        .unwrap();
    assert_eq!(escrow_balance, wager_amount);

    let session_acc = ctx
        .banks_client
        .get_account(session_pda)
        .await
        .unwrap()
        .expect("session delegation account missing");
    let session = GlobalSessionDelegation::try_deserialize(&mut &session_acc.data[..]).unwrap();
    assert_eq!(session.total_spent, wager_amount);
    assert_eq!(session.games_remaining, 199);
}

/// docs/PRE_MAINNET_E2E_PLAN.md §2.2: `platform_fee` used to have no on-chain
/// bound at all — a buggy or malicious caller could set `game.country_fee`
/// high enough to consume most/all of the wager pot at settlement. This
/// proves `common::init_game_fields`'s new `MAX_PLATFORM_FEE_LAMPORTS` check
/// actually rejects an absurd value instead of silently accepting it.
#[tokio::test]
async fn global_create_game_rejects_unreasonable_platform_fee() {
    let player = Pubkey::new_unique();
    let session_signer = Keypair::new();
    let wager_amount = 10_000_000u64;

    let (session_pda, session_account_data) = global_session_account(
        player,
        session_signer.pubkey(),
        5_000_000_000,
        1_000_000_000,
    );

    let mut ctx = start(vec![(session_pda, session_account_data)]).await;

    let game_id = 43u64;
    let absurd_platform_fee = 2_000_000_000u64; // 2 SOL — far above any real per-game fee
    let ix = global_create_game_ix(
        session_pda,
        session_signer.pubkey(),
        player,
        game_id,
        wager_amount,
        MatchType::Rated,
        absurd_platform_fee,
        300,
        0,
    );

    let err = send(&mut ctx, ix, &[&session_signer])
        .await
        .expect_err("an absurdly large platform_fee must be rejected");

    assert_eq!(
        custom_code(&err),
        Some(ec(xfchess_game::errors::GameErrorCode::PlatformFeeTooLarge)),
        "expected PlatformFeeTooLarge, got {err:?}"
    );
}

#[tokio::test]
async fn global_create_game_works_for_a_free_zero_wager_game() {
    // Rent still has to be funded by the PDA vault even with no wager — this
    // is the code path that would fail on the very first game creation for
    // any brand-new session, wagered or not.
    let player = Pubkey::new_unique();
    let session_signer = Keypair::new();

    let (session_pda, session_account_data) = global_session_account(
        player,
        session_signer.pubkey(),
        5_000_000_000,
        1_000_000_000,
    );

    let mut ctx = start(vec![(session_pda, session_account_data)]).await;

    let game_id = 7u64;
    let ix = global_create_game_ix(
        session_pda,
        session_signer.pubkey(),
        player,
        game_id,
        0,
        MatchType::Free,
        0,
        300,
        0,
    );

    send(&mut ctx, ix, &[&session_signer])
        .await
        .expect("global_create_game should succeed for a zero-wager game");

    let game = fetch_game(&mut ctx, game_id).await;
    assert_eq!(game.wager_amount, 0);
    assert_eq!(game.status, GameStatus::WaitingForOpponent);
}

// ── global_join_game: same bug, join side ──────────────────────────────────
// `global_join_game`'s wager transfer had the identical "from must not carry
// data" bug as `global_create_game` (both drew from the same
// `GlobalSessionDelegation` PDA vault via a plain `system_program::transfer`),
// just never caught because nobody exercised it under the real BPF runtime.

fn player_profile_pda(player: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"profile", player.as_ref()], &xfchess_game::ID).0
}

fn player_profile_account(authority: Pubkey) -> (Pubkey, solana_sdk::account::Account) {
    let pda = player_profile_pda(&authority);
    let profile = PlayerProfile {
        authority,
        ..Default::default()
    };
    (
        pda,
        program_account(&profile, 8 + PlayerProfile::INIT_SPACE),
    )
}

/// A `Game` waiting for an opponent, with a wager already escrowed by the
/// creator — the state `global_join_game` expects to find on-chain.
fn waiting_game_account(
    game_id: u64,
    white: Pubkey,
    wager_amount: u64,
) -> (Pubkey, solana_sdk::account::Account) {
    let (pda, bump) = game_pda(game_id);
    let game = Game {
        game_id,
        white,
        black: Pubkey::default(),
        status: GameStatus::WaitingForOpponent,
        last_move_timestamp: 0,
        fees_advanced: 0,
        fee_payer: white,
        result: GameResult::None,
        board_state: start_board(),
        move_count: 0,
        halfmove_clock: 0,
        turn: 1,
        created_at: 0,
        updated_at: 0,
        wager_amount,
        wager_token: None,
        game_type: GameType::PvP,
        match_type: MatchType::Rated,
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

fn global_join_game_ix(
    session_pda: Pubkey,
    session_signer: Pubkey,
    player: Pubkey,
    white: Pubkey,
    game_id: u64,
) -> Instruction {
    let accounts = xfchess_game::__client_accounts_global_join_game::GlobalJoinGame {
        session_delegation: session_pda,
        session_signer,
        player,
        game: game_pda(game_id).0,
        player_profile: player_profile_pda(&player),
        white_profile: player_profile_pda(&white),
        escrow_pda: escrow_pda(game_id),
        system_program: solana_system_interface::program::ID,
    }
    .to_account_metas(None);
    let data = xfchess_game::instruction::GlobalJoinGame { game_id }.data();
    Instruction {
        program_id: xfchess_game::ID,
        accounts,
        data,
    }
}

#[tokio::test]
async fn global_join_game_funds_wager_from_session_vault() {
    let white = Pubkey::new_unique();
    let black = Pubkey::new_unique();
    let session_signer = Keypair::new();
    let game_id = 99u64;
    let wager_amount = 20_000_000u64;

    let (session_pda, session_account_data) =
        global_session_account(black, session_signer.pubkey(), 5_000_000_000, 1_000_000_000);
    let (game_pda_key, game_account_data) = waiting_game_account(game_id, white, wager_amount);
    let (white_profile_pda, white_profile_data) = player_profile_account(white);
    let (black_profile_pda, black_profile_data) = player_profile_account(black);

    let mut ctx = start(vec![
        (session_pda, session_account_data),
        (game_pda_key, game_account_data),
        (white_profile_pda, white_profile_data),
        (black_profile_pda, black_profile_data),
    ])
    .await;

    let ix = global_join_game_ix(session_pda, session_signer.pubkey(), black, white, game_id);
    send(&mut ctx, ix, &[&session_signer])
        .await
        .expect("global_join_game should succeed when the session vault PDA funds the wager");

    let game = fetch_game(&mut ctx, game_id).await;
    assert_eq!(game.black, black);
    assert_eq!(game.status, GameStatus::Active);

    let escrow_balance = ctx
        .banks_client
        .get_balance(escrow_pda(game_id))
        .await
        .unwrap();
    assert_eq!(escrow_balance, wager_amount);

    let session_acc = ctx
        .banks_client
        .get_account(session_pda)
        .await
        .unwrap()
        .expect("session delegation account missing");
    let session = GlobalSessionDelegation::try_deserialize(&mut &session_acc.data[..]).unwrap();
    assert_eq!(session.total_spent, wager_amount);
    assert_eq!(session.games_remaining, 199);
}
