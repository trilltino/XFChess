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
async fn test_platform_fee_collection() {
    let program_id = Pubkey::new_unique();
    let mut program_test = ProgramTest::new(
        "xfchess_game",
        program_id,
        processor!(xfchess_game::entry),
    );

    let player = Keypair::new();
    let tournament_id = 1u64;
    let tournament_pda, _ = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    );
    let treasury_vault = Keypair::new();
    let platform_treasury_vault = Keypair::new();

    program_test.add_account(
        player.pubkey(),
        Account {
            lamports: 1_000_000_000,
            ..Account::default()
        },
    );
    program_test.add_account(
        treasury_vault.pubkey(),
        Account {
            lamports: 1_000_000_000,
            ..Account::default()
        },
    );
    program_test.add_account(
        platform_treasury_vault.pubkey(),
        Account {
            lamports: 0,
            ..Account::default()
        },
    );

    let mut ctx = program_test.start_with_context().await;

    // Initialize tournament
    let init_tournament_ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(treasury_vault.pubkey(), true),
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new_readonly(solana_sdk::system_program::ID, false),
        ],
        data: InitializeTournament {
            tournament_id,
            name: "Test Tournament".to_string(),
            entry_fee: 1_000_000,
            prize_pool: 10_000_000,
            max_players: 16,
            prize_shares: [4000, 2500, 1500, 1000, 500, 500, 0, 0, 0, 0],
        }.data(),
    };

    let tx = Transaction::new_signed_with_payer(
        &[init_tournament_ix],
        Some(&treasury_vault.pubkey()),
        &[&treasury_vault],
        ctx.last_blockhash,
    );
    ctx.banks_client.process_transaction(tx).await.unwrap();

    // Register player with platform fee
    let register_ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(player.pubkey(), true),
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new(treasury_vault.pubkey(), false),
            AccountMeta::new(platform_treasury_vault.pubkey(), false),
            AccountMeta::new_readonly(solana_sdk::system_program::ID, false),
        ],
        data: RegisterPlayer {
            tournament_id,
            elo: 1200,
        }.data(),
    };

    let tx = Transaction::new_signed_with_payer(
        &[register_ix],
        Some(&player.pubkey()),
        &[&player],
        ctx.last_blockhash,
    );
    ctx.banks_client.process_transaction(tx).await.unwrap();

    // Check if platform fee was collected
    let treasury_account = ctx.banks_client.get_account(platform_treasury_vault.pubkey()).await.unwrap().unwrap();
    assert_eq!(treasury_account.lamports, PLATFORM_FEE_LAMPORTS, "Platform fee should be collected");
}

#[tokio::test]
async fn test_rent_refund_on_tournament_close() {
    let program_id = Pubkey::new_unique();
    let mut program_test = ProgramTest::new(
        "xfchess_game",
        program_id,
        processor!(xfchess_game::entry),
    );

    let authority = Keypair::new();
    let player1 = Keypair::new();
    let player2 = Keypair::new();
    let tournament_id = 2u64;
    let tournament_pda, _ = Pubkey::find_program_address(
        &[TOURNAMENT_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    );
    let prize_escrow_pda, _ = Pubkey::find_program_address(
        &[TOURNAMENT_PRIZE_ESCROW_SEED, &tournament_id.to_le_bytes()],
        &program_id,
    );
    let treasury_vault = Keypair::new();

    program_test.add_account(
        authority.pubkey(),
        Account {
            lamports: 1_000_000_000,
            ..Account::default()
        },
    );
    program_test.add_account(
        player1.pubkey(),
        Account {
            lamports: 1_000_000_000,
            ..Account::default()
        },
    );
    program_test.add_account(
        player2.pubkey(),
        Account {
            lamports: 1_000_000_000,
            ..Account::default()
        },
    );
    program_test.add_account(
        treasury_vault.pubkey(),
        Account {
            lamports: 1_000_000_000,
            ..Account::default()
        },
    );
    program_test.add_account(
        prize_escrow_pda,
        Account {
            lamports: 10_000_000_000, // Simulate accumulated fees
            ..Account::default()
        },
    );

    let mut ctx = program_test.start_with_context().await;

    // Initialize tournament
    let init_tournament_ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(treasury_vault.pubkey(), true),
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new_readonly(solana_sdk::system_program::ID, false),
        ],
        data: InitializeTournament {
            tournament_id,
            name: "Test Tournament".to_string(),
            entry_fee: 1_000_000,
            prize_pool: 10_000_000,
            max_players: 16,
            prize_shares: [4000, 2500, 1500, 1000, 500, 500, 0, 0, 0, 0],
        }.data(),
    };

    let tx = Transaction::new_signed_with_payer(
        &[init_tournament_ix],
        Some(&treasury_vault.pubkey()),
        &[&treasury_vault],
        ctx.last_blockhash,
    );
    ctx.banks_client.process_transaction(tx).await.unwrap();

    // Register players
    let register_player1_ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(player1.pubkey(), true),
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new(treasury_vault.pubkey(), false),
            AccountMeta::new(treasury_vault.pubkey(), false), // Simulating platform_treasury_vault
            AccountMeta::new_readonly(solana_sdk::system_program::ID, false),
        ],
        data: RegisterPlayer {
            tournament_id,
            elo: 1200,
        }.data(),
    };

    let tx = Transaction::new_signed_with_payer(
        &[register_player1_ix],
        Some(&player1.pubkey()),
        &[&player1],
        ctx.last_blockhash,
    );
    ctx.banks_client.process_transaction(tx).await.unwrap();

    let register_player2_ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(player2.pubkey(), true),
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new(treasury_vault.pubkey(), false),
            AccountMeta::new(treasury_vault.pubkey(), false), // Simulating platform_treasury_vault
            AccountMeta::new_readonly(solana_sdk::system_program::ID, false),
        ],
        data: RegisterPlayer {
            tournament_id,
            elo: 1300,
        }.data(),
    };

    let tx = Transaction::new_signed_with_payer(
        &[register_player2_ix],
        Some(&player2.pubkey()),
        &[&player2],
        ctx.last_blockhash,
    );
    ctx.banks_client.process_transaction(tx).await.unwrap();

    // Close tournament and check rent refunds
    let close_tournament_ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(authority.pubkey(), true),
            AccountMeta::new(tournament_pda, false),
            AccountMeta::new(prize_escrow_pda, false),
            AccountMeta::new(treasury_vault.pubkey(), false),
            AccountMeta::new(player1.pubkey(), false), // Winner
            AccountMeta::new(player2.pubkey(), false), // Second place
            AccountMeta::new(player1.pubkey(), false), // Placeholder for others
            AccountMeta::new(player1.pubkey(), false),
            AccountMeta::new(player1.pubkey(), false),
            AccountMeta::new(player1.pubkey(), false),
            AccountMeta::new(player1.pubkey(), false),
            AccountMeta::new(player1.pubkey(), false),
            AccountMeta::new(player1.pubkey(), false),
            AccountMeta::new(player1.pubkey(), false),
            AccountMeta::new_readonly(solana_sdk::system_program::ID, false),
        ],
        data: CloseTournament {
            tournament_id,
        }.data(),
    };

    let tx = Transaction::new_signed_with_payer(
        &[close_tournament_ix],
        Some(&authority.pubkey()),
        &[&authority],
        ctx.last_blockhash,
    );
    ctx.banks_client.process_transaction(tx).await.unwrap();

    // Verify rent refund distribution
    let player1_account = ctx.banks_client.get_account(player1.pubkey()).await.unwrap().unwrap();
    let player2_account = ctx.banks_client.get_account(player2.pubkey()).await.unwrap().unwrap();
    assert!(player1_account.lamports > 1_000_000_000, "Player 1 should receive rent refund");
    assert!(player2_account.lamports > 1_000_000_000, "Player 2 should receive rent refund");
}
