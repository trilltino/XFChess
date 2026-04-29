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

#[tokio::test]
async fn test_idempotency_finalize() {
    let program_id = Pubkey::new_unique();
    let mut program_test = ProgramTest::new(
        "xfchess_game",
        program_id,
        processor!(xfchess_game::entry),
    );

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

    // Create game
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

    // Finalize game once
    let finalize_ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(white_player.pubkey(), true),
            AccountMeta::new(black_player.pubkey(), false),
            AccountMeta::new(game_pda, false),
        ],
        data: Finalize {
            game_id,
            result: GameResult::Winner(white_player.pubkey()),
        }.data(),
    };

    let tx = Transaction::new_signed_with_payer(
        &[finalize_ix],
        Some(&white_player.pubkey()),
        &[&white_player],
        ctx.last_blockhash,
    );
    ctx.banks_client.process_transaction(tx).await.unwrap();

    // Attempt to finalize again (should fail due to idempotency)
    let tx = Transaction::new_signed_with_payer(
        &[finalize_ix],
        Some(&white_player.pubkey()),
        &[&white_player],
        ctx.last_blockhash,
    );
    let result = ctx.banks_client.process_transaction(tx).await;
    assert!(result.is_err(), "Second finalize should fail due to idempotency");
}

#[tokio::test]
async fn test_side_to_move_enforcement() {
    let program_id = Pubkey::new_unique();
    let mut program_test = ProgramTest::new(
        "xfchess_game",
        program_id,
        processor!(xfchess_game::entry),
    );

    let white_player = Keypair::new();
    let black_player = Keypair::new();
    let game_id = 2u64;
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

    // Create game
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

    // Attempt move by black player first (should fail as white moves first)
    let move_ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(black_player.pubkey(), true),
            AccountMeta::new(game_pda, false),
        ],
        data: RecordMove {
            game_id,
            move_fen: "e4".to_string(),
        }.data(),
    };

    let tx = Transaction::new_signed_with_payer(
        &[move_ix],
        Some(&black_player.pubkey()),
        &[&black_player],
        ctx.last_blockhash,
    );
    let result = ctx.banks_client.process_transaction(tx).await;
    assert!(result.is_err(), "Black player should not be able to move first");
}
