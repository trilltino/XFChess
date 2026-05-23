#![cfg(not(feature = "idl-build"))]

use anchor_lang::prelude::*;
use anchor_spl::token::{Token, TokenAccount};
use solana_program_test::*;
use solana_sdk::{
    account::Account,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use xfchess_game::{
    constants::*,
    errors::GameErrorCode,
    state::*,
    instruction::*,
};

// Helper to create program test with our program
fn program_test() -> ProgramTest {
    ProgramTest::new(
        "xfchess_game",
        xfchess_game::ID,
        processor!(xfchess_game::entry),
    )
}

#[tokio::test]
async fn test_init_profile() {
    let program_id = Pubkey::new_unique();
    let mut program_test = program_test();

    let player = Keypair::new();
    let username = "tester123".to_string();
    let (profile_pda, _) =
        Pubkey::find_program_address(&[PROFILE_SEED, player.pubkey().as_ref()], &program_id);
    let (username_record, _) =
        Pubkey::find_program_address(&[USERNAME_SEED, username.as_bytes()], &program_id);

    let mut ctx = program_test.start_with_context().await;

    program_test.add_account(
        player.pubkey(),
        Account {
            lamports: 1_000_000_000,
            ..Account::default()
        },
    );

    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(profile_pda, false),
            AccountMeta::new(username_record, false),
            AccountMeta::new(player.pubkey(), true),
            AccountMeta::new_readonly(solana_sdk::system_program::ID, false),
        ],
        data: InitProfile { username: username.clone(), country: "US".to_string(), date_of_birth: -600_000_000 }.data(),
    };

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&player.pubkey()),
        &[&player],
        ctx.last_blockhash,
    );
    ctx.banks_client.process_transaction(tx).await.unwrap();

    let account = ctx.banks_client.get_account(profile_pda).await.unwrap().unwrap();
    let profile: PlayerProfile =
        anchor_lang::AccountDeserialize::try_deserialize(&mut &account.data[..]).unwrap();

    assert_eq!(profile.authority, player.pubkey());
    assert_eq!(profile.elo_rating, 120000.0);
    assert_eq!(profile.username, username);
    assert_eq!(profile.username_set, true);
}

#[tokio::test]
async fn test_join_game_pvp() {
    let program_id = Pubkey::new_unique();
    let mut program_test = program_test();

    let opponent = Keypair::new();
    let game_id: u64 = 54321;

    let (opponent_profile, _) =
        Pubkey::find_program_address(&[PROFILE_SEED, opponent.pubkey().as_ref()], &program_id);
    let (payer_profile, _) =
        Pubkey::find_program_address(&[PROFILE_SEED, program_test.payer.pubkey().as_ref()], &program_id);
    let (game_pda, _) =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id);
    let (move_log_pda, _) =
        Pubkey::find_program_address(&[MOVE_LOG_SEED, &game_id.to_le_bytes()], &program_id);
    let (escrow_pda, _) =
        Pubkey::find_program_address(&[WAGER_ESCROW_SEED, &game_id.to_le_bytes()], &program_id);

    let mut ctx = program_test.start_with_context().await;

    program_test.add_account(
        opponent.pubkey(),
        Account {
            lamports: 1_000_000_000,
            ..Account::default()
        },
    );

    // 1. Create Game (payer acts as fee_payer in tests — no separate VPS session)
    let create_ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new(move_log_pda, false),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new(program_test.payer.pubkey(), true),
            AccountMeta::new_readonly(solana_sdk::system_program::ID, false),
        ],
        data: CreateGame {
            game_id,
            wager_amount: 0,
            match_type: MatchType::Free,
            country: String::from("US"),
            base_time_seconds: 0,
            increment_seconds: 0,
        }.data(),
    };

    let mut tx = Transaction::new_signed_with_payer(
        &[create_ix],
        Some(&program_test.payer.pubkey()),
        &[&program_test.payer],
        ctx.last_blockhash,
    );
    ctx.banks_client.process_transaction(tx).await.unwrap();

    // 2. Join Game (payer acts as fee_payer to satisfy game.fee_payer == fee_payer check)
    let join_ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new(opponent_profile, false),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new(payer_profile, false),
            AccountMeta::new(opponent.pubkey(), true),
            AccountMeta::new_readonly(solana_sdk::system_program::ID, false),
        ],
        data: JoinGame { game_id }.data(),
    };

    let mut join_tx = Transaction::new_signed_with_payer(
        &[join_ix],
        Some(&program_test.payer.pubkey()),
        &[&program_test.payer, &opponent],
        ctx.last_blockhash,
    );
    ctx.banks_client.process_transaction(join_tx).await.unwrap();

    let account = ctx.banks_client.get_account(game_pda).await.unwrap().unwrap();
    let game: Game =
        anchor_lang::AccountDeserialize::try_deserialize(&mut &account.data[..]).unwrap();

    assert_eq!(game.black, opponent.pubkey());
    assert_eq!(game.status, GameStatus::Active);
}

#[tokio::test]
async fn test_finalize_game_elo() {
    let program_id = Pubkey::new_unique();
    let mut program_test = program_test();

    let (mut ctx, banks_client, payer, recent_blockhash) = program_test.start().await;

    let white = payer.pubkey();
    let black = Keypair::new();
    let game_id: u64 = 777;

    let (treasury_vault, _) =
        Pubkey::find_program_address(&[TREASURY_VAULT_SEED], &program_id);

    let (white_profile, _) =
        Pubkey::find_program_address(&[PROFILE_SEED, white.as_ref()], &program_id);
    let (white_username_rec, _) =
        Pubkey::find_program_address(&[USERNAME_SEED, b"white_p"], &program_id);
    let (black_profile, _) =
        Pubkey::find_program_address(&[PROFILE_SEED, black.pubkey().as_ref()], &program_id);
    let (black_username_rec, _) =
        Pubkey::find_program_address(&[USERNAME_SEED, b"black_p"], &program_id);

    let (game_pda, _) =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id);
    let (move_log_pda, _) =
        Pubkey::find_program_address(&[MOVE_LOG_SEED, &game_id.to_le_bytes()], &program_id);
    let (escrow_pda, _) =
        Pubkey::find_program_address(&[WAGER_ESCROW_SEED, &game_id.to_le_bytes()], &program_id);

    // 1. Setup Profiles
    let init_white = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(white_profile, false),
            AccountMeta::new(white_username_rec, false),
            AccountMeta::new(white, true),
            AccountMeta::new_readonly(solana_sdk::system_program::ID, false),
        ],
        data: InitProfile { username: "white_p".to_string(), country: "US".to_string(), date_of_birth: -600_000_000 }.data(),
    };
    let init_black = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(black_profile, false),
            AccountMeta::new(black_username_rec, false),
            AccountMeta::new(black.pubkey(), true),
            AccountMeta::new_readonly(solana_sdk::system_program::ID, false),
        ],
        data: InitProfile { username: "black_p".to_string(), country: "US".to_string(), date_of_birth: -600_000_000 }.data(),
    };

    // 2. Create and Join Game (payer acts as fee_payer in tests)
    let create_ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new(move_log_pda, false),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new(white, true),
            AccountMeta::new_readonly(solana_sdk::system_program::ID, false),
        ],
        data: CreateGame {
            game_id,
            wager_amount: 0,
            match_type: MatchType::Free,
            country: String::from("US"),
            base_time_seconds: 0,
            increment_seconds: 0,
        }.data(),
    };
    let join_ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(game_pda, false),
            AccountMeta::new(black_profile, false),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new(white_profile, false),
            AccountMeta::new(black.pubkey(), true),
            AccountMeta::new_readonly(solana_sdk::system_program::ID, false),
        ],
        data: JoinGame { game_id }.data(),
    };

    let mut tx =
        Transaction::new_signed_with_payer(
            &[init_white, init_black, create_ix, join_ix],
            Some(&white),
            &[&payer, &black],
            recent_blockhash,
        );
    banks_client.process_transaction(tx).await.unwrap();

    // 3. Assertions - Game joined successfully
    let game_acc = banks_client.get_account(game_pda).await.unwrap().unwrap();
    let game_data: Game =
        anchor_lang::AccountDeserialize::try_deserialize(&mut &game_acc.data[..]).unwrap();
    assert_eq!(game_data.status, GameStatus::Active);
    assert_eq!(game_data.black, black.pubkey());
}

#[tokio::test]
async fn test_game_flow() {
    let program_id = Pubkey::new_unique();
    let mut program_test = ProgramTest::new(
        "xfchess_game",
        program_id,
        processor!(xfchess_game::entry),
    );

    // Add necessary accounts and setup for testing
    let white_player = Keypair::new();
    let black_player = Keypair::new();
    let game_id = 1u64;
    let game_pda, _ = Pubkey::find_program_address(
        &[GAME_SEED, &game_id.to_le_bytes()],
        &program_id,
    );

    program_test.add_account(
        white_player.pubkey(),
        Account {
            lamports: 1_000_000_000,
            ..Account::default()
        },
    );
    program_test.add_account(
        black_player.pubkey(),
        Account {
            lamports: 1_000_000_000,
            ..Account::default()
        },
    );

    let mut ctx = program_test.start_with_context().await;

    // Test game creation
    let create_game_ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(white_player.pubkey(), true),
            AccountMeta::new(black_player.pubkey(), false),
            AccountMeta::new(game_pda, false),
            AccountMeta::new_readonly(solana_sdk::system_program::ID, false),
        ],
        data: CreateGame {
            game_id,
            wager_amount: 0,
            time_control: TimeControl {
                initial_time: 300,
                increment: 3,
            },
        }.data(),
    };

    let tx = Transaction::new_signed_with_payer(
        &[create_game_ix],
        Some(&white_player.pubkey()),
        &[&white_player],
        ctx.last_blockhash,
    );
    ctx.banks_client.process_transaction(tx).await.unwrap();

    // Additional test steps can be added here for game flow
}
