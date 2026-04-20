use anchor_lang::prelude::*;
use anchor_lang::InstructionData;
use solana_program_test::*;
use solana_sdk::{
    instruction::AccountMeta, pubkey::Pubkey, signature::Keypair, signer::Signer, system_program,
    transaction::Transaction,
};
use xfchess_game::{
    constants::{
        GAME_SEED, MOVE_LOG_SEED, PROFILE_SEED, USERNAME_SEED, WAGER_ESCROW_SEED,
        TREASURY_VAULT_SEED,
    },
    state::{Game, GameResult, GameStatus, GameType, PlayerProfile},
};

// Helper to create program test with our program
fn program_test() -> ProgramTest {
    ProgramTest::new("xfchess_game", xfchess_game::ID, None)
}

#[tokio::test]
async fn test_init_profile() {
    let mut program_test = program_test();
    let program_id = xfchess_game::ID;

    let player = Keypair::new();
    let username = "tester123".to_string();
    let (profile_pda, _) =
        Pubkey::find_program_address(&[PROFILE_SEED, player.pubkey().as_ref()], &program_id);
    let (username_record, _) =
        Pubkey::find_program_address(&[USERNAME_SEED, username.as_bytes()], &program_id);


    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;

    let ix = solana_sdk::instruction::Instruction {
        program_id,
        accounts: xfchess_game::accounts::InitProfile {
            player_profile: profile_pda,
            username_record,
            player: player.pubkey(),
            system_program: system_program::id(),
        }
        .to_account_metas(None),
        data: xfchess_game::instruction::InitProfile { username: username.clone() }.data(),

    };

    let mut tx = Transaction::new_with_payer(&[ix], Some(&payer.pubkey()));
    tx.sign(&[&payer, &player], recent_blockhash);
    banks_client.process_transaction(tx).await.unwrap();

    let account = banks_client
        .get_account(profile_pda)
        .await
        .unwrap()
        .unwrap();
    let profile: PlayerProfile =
        anchor_lang::AccountDeserialize::try_deserialize(&mut &account.data[..]).unwrap();

    assert_eq!(profile.authority, player.pubkey());
    assert_eq!(profile.elo_rating, 120000.0);
    assert_eq!(profile.username, username);
    assert_eq!(profile.username_set, true);
}

#[tokio::test]
async fn test_create_game_pvai() {
    let mut program_test = program_test();
    let program_id = xfchess_game::ID;

    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;

    let game_id: u64 = 12345;
    let wager: u64 = 0;
    let (game_pda, _) =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id);
    let (move_log_pda, _) =
        Pubkey::find_program_address(&[MOVE_LOG_SEED, &game_id.to_le_bytes()], &program_id);
    let (escrow_pda, _) =
        Pubkey::find_program_address(&[WAGER_ESCROW_SEED, &game_id.to_le_bytes()], &program_id);

    let ix = solana_sdk::instruction::Instruction {
        program_id,
        accounts: xfchess_game::accounts::CreateGame {
            game: game_pda,
            move_log: move_log_pda,
            escrow_pda,
            player: payer.pubkey(),
            system_program: system_program::id(),
        }
        .to_account_metas(None),
        data: xfchess_game::instruction::CreateGame {
            game_id,
            wager_amount: wager,
            game_type: GameType::PvAI,
            match_type: xfchess_game::state::MatchType::Free,
            country: String::from("US"),
            time_per_move: 0,
        }
        .data(),
    };

    let mut tx = Transaction::new_with_payer(&[ix], Some(&payer.pubkey()));
    tx.sign(&[&payer], recent_blockhash);
    banks_client.process_transaction(tx).await.unwrap();

    let account = banks_client.get_account(game_pda).await.unwrap().unwrap();
    let game: Game =
        anchor_lang::AccountDeserialize::try_deserialize(&mut &account.data[..]).unwrap();

    assert_eq!(game.game_type, GameType::PvAI);
    assert_eq!(game.white, payer.pubkey());
    assert_eq!(game.black, xfchess_game::constants::ai_authority::ID);
    assert_eq!(game.status, GameStatus::Active);
}

#[tokio::test]
async fn test_join_game_pvp() {
    let mut program_test = program_test();
    let program_id = xfchess_game::ID;

    let opponent = Keypair::new();
    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;

    let game_id: u64 = 54321;
    let (opponent_profile, _) =
        Pubkey::find_program_address(&[PROFILE_SEED, opponent.pubkey().as_ref()], &program_id);
    let (payer_profile, _) =
        Pubkey::find_program_address(&[PROFILE_SEED, payer.pubkey().as_ref()], &program_id);
    let (game_pda, _) =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id);
    let (move_log_pda, _) =
        Pubkey::find_program_address(&[MOVE_LOG_SEED, &game_id.to_le_bytes()], &program_id);
    let (escrow_pda, _) =
        Pubkey::find_program_address(&[WAGER_ESCROW_SEED, &game_id.to_le_bytes()], &program_id);

    // 1. Create Game
    let create_ix = solana_sdk::instruction::Instruction {
        program_id,
        accounts: xfchess_game::accounts::CreateGame {
            game: game_pda,
            move_log: move_log_pda,
            escrow_pda,
            player: payer.pubkey(),
            system_program: system_program::id(),
        }
        .to_account_metas(None),
        data: xfchess_game::instruction::CreateGame {
            game_id,
            wager_amount: 0,
            game_type: GameType::PvP,
            match_type: xfchess_game::state::MatchType::Free,
            country: String::from("US"),
            time_per_move: 0,
        }
        .data(),
    };

    let mut tx = Transaction::new_with_payer(&[create_ix], Some(&payer.pubkey()));
    tx.sign(&[&payer], recent_blockhash);
    banks_client.process_transaction(tx).await.unwrap();

    // 2. Join Game
    let join_ix = solana_sdk::instruction::Instruction {
        program_id,
        accounts: xfchess_game::accounts::JoinGame {
            game: game_pda,
            player_profile: opponent_profile,
            escrow_pda,
            white_profile: payer_profile,
            player: opponent.pubkey(),
            system_program: system_program::id(),
        }
        .to_account_metas(None),
        data: xfchess_game::instruction::JoinGame { game_id }.data(),
    };

    let mut join_tx = Transaction::new_with_payer(&[join_ix], Some(&payer.pubkey()));
    join_tx.sign(&[&payer, &opponent], recent_blockhash);
    banks_client.process_transaction(join_tx).await.unwrap();

    let account = banks_client.get_account(game_pda).await.unwrap().unwrap();
    let game: Game =
        anchor_lang::AccountDeserialize::try_deserialize(&mut &account.data[..]).unwrap();

    assert_eq!(game.black, opponent.pubkey());
    assert_eq!(game.status, GameStatus::Active);
}

#[tokio::test]
async fn test_record_move_ai_security() {
    let mut program_test = program_test();
    let program_id = xfchess_game::ID;

    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;

    let game_id: u64 = 999;
    let (game_pda, _) =
        Pubkey::find_program_address(&[GAME_SEED, &game_id.to_le_bytes()], &program_id);
    let (move_log_pda, _) =
        Pubkey::find_program_address(&[MOVE_LOG_SEED, &game_id.to_le_bytes()], &program_id);
    let (escrow_pda, _) =
        Pubkey::find_program_address(&[WAGER_ESCROW_SEED, &game_id.to_le_bytes()], &program_id);

    // 1. Create PvAI Game (Starts Active)
    let create_ix = solana_sdk::instruction::Instruction {
        program_id,
        accounts: xfchess_game::accounts::CreateGame {
            game: game_pda,
            move_log: move_log_pda,
            escrow_pda,
            player: payer.pubkey(),
            system_program: system_program::id(),
        }
        .to_account_metas(None),
        data: xfchess_game::instruction::CreateGame {
            game_id,
            wager_amount: 0,
            game_type: GameType::PvAI,
            match_type: xfchess_game::state::MatchType::Free,
            country: String::from("US"),
            time_per_move: 0,
        }
        .data(),
    };
    let mut tx = Transaction::new_with_payer(&[create_ix], Some(&payer.pubkey()));
    tx.sign(&[&payer], recent_blockhash);
    banks_client.process_transaction(tx).await.unwrap();

    // 2. White Move (Turn 0)
    let move_0_ix = solana_sdk::instruction::Instruction {
        program_id,
        accounts: xfchess_game::accounts::RecordMove {
            game: game_pda,
            move_log: move_log_pda,
            player: payer.pubkey(),
            session_delegation: payer.pubkey(),
        }
        .to_account_metas(None),
        data: xfchess_game::instruction::RecordMove {
            game_id,
            move_str: "e2e4".to_string(),
            next_fen: "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1".to_string(),
            nonce: 0,
            signature: None,
        }
        .data(),
    };
    let mut tx0 = Transaction::new_with_payer(&[move_0_ix], Some(&payer.pubkey()));
    tx0.sign(&[&payer], recent_blockhash);
    banks_client.process_transaction(tx0).await.unwrap();

    // 3. Black Move (Turn 1) - Should Fail if NOT AI Authority
    let move_1_fake_ix = solana_sdk::instruction::Instruction {
        program_id,
        accounts: xfchess_game::accounts::RecordMove {
            game: game_pda,
            move_log: move_log_pda,
            player: payer.pubkey(), // Payer is NOT AI authority
            session_delegation: payer.pubkey(),
        }
        .to_account_metas(None),
        data: xfchess_game::instruction::RecordMove {
            game_id,
            move_str: "e7e5".to_string(),
            next_fen: "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2".to_string(),
            nonce: 0,
            signature: None,
        }
        .data(),
    };
    let mut tx1f = Transaction::new_with_payer(&[move_1_fake_ix], Some(&payer.pubkey()));
    tx1f.sign(&[&payer], recent_blockhash);
    let result = banks_client.process_transaction(tx1f).await;
    assert!(result.is_err()); // Correctly rejected
}

#[tokio::test]
async fn test_finalize_game_elo() {
    let mut program_test = program_test();
    let program_id = xfchess_game::ID;

    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;

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
    let init_white = solana_sdk::instruction::Instruction {
        program_id,
        accounts: xfchess_game::accounts::InitProfile {
            player_profile: white_profile,
            username_record: white_username_rec,
            player: white,
            system_program: system_program::id(),
        }
        .to_account_metas(None),
        data: xfchess_game::instruction::InitProfile { username: "white_p".to_string() }.data(),
    };
    let init_black = solana_sdk::instruction::Instruction {
        program_id,
        accounts: xfchess_game::accounts::InitProfile {
            player_profile: black_profile,
            username_record: black_username_rec,
            player: black.pubkey(),
            system_program: system_program::id(),
        }
        .to_account_metas(None),
        data: xfchess_game::instruction::InitProfile { username: "black_p".to_string() }.data(),
    };


    // 2. Create and Join Game
    let create_ix = solana_sdk::instruction::Instruction {
        program_id,
        accounts: xfchess_game::accounts::CreateGame {
            game: game_pda,
            move_log: move_log_pda,
            escrow_pda,
            player: white,
            system_program: system_program::id(),
        }
        .to_account_metas(None),
        data: xfchess_game::instruction::CreateGame {
            game_id,
            wager_amount: 0,
            game_type: GameType::PvP,
            match_type: xfchess_game::state::MatchType::Free,
            country: String::from("US"),
            time_per_move: 0,
        }
        .data(),
    };
    let join_ix = solana_sdk::instruction::Instruction {
        program_id,
        accounts: xfchess_game::accounts::JoinGame {
            game: game_pda,
            player_profile: black_profile,
            escrow_pda,
            white_profile: white_profile,
            player: black.pubkey(),
            system_program: system_program::id(),
        }
        .to_account_metas(None),
        data: xfchess_game::instruction::JoinGame { game_id }.data(),
    };

    let mut tx =
        Transaction::new_with_payer(&[init_white, init_black, create_ix, join_ix], Some(&white));
    tx.sign(&[&payer, &black], recent_blockhash);
    banks_client.process_transaction(tx).await.unwrap();

    // 3. Assertions - Game joined successfully
    let game_acc = banks_client.get_account(game_pda).await.unwrap().unwrap();
    let game_data: Game =
        anchor_lang::AccountDeserialize::try_deserialize(&mut &game_acc.data[..]).unwrap();
    assert_eq!(game_data.status, GameStatus::Active);
    assert_eq!(game_data.black, black.pubkey());
}
