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
    (pda, program_account(&profile, 8 + PlayerProfile::INIT_SPACE))
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
    };
    (pda, program_account(&game, 8 + Game::INIT_SPACE))
}

fn finalize_game_ix(
    game_id: u64,
    white: Pubkey,
    black: Pubkey,
    fee_payer: Pubkey,
) -> Instruction {
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

    let (game_pda_key, game_account_data) =
        finished_game_account(game_id, white, black, fee_payer.pubkey(), wager_amount, country_fee);
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
    assert!(closed.is_none(), "Game account should be closed after finalize_game");
}
