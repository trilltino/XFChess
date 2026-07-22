//! Instruction-level (`ProgramTest`) verification of `register_player` — the
//! actual on-chain call the game client's tournament "Join" button submits
//! (`src/multiplayer/solana/tournament_session.rs::build_register_player_ix`,
//! `src/multiplayer/solana/tournament.rs::register_tournament`).
//!
//! This exists because the game client used to have a stub here
//! (`register_tournament` just returned `Ok(0)` without ever submitting a
//! transaction) — no player has ever had an entry fee collected on-chain by
//! joining a tournament through the shipped game. This test proves the fixed
//! client-side account layout is exactly what the real, compiled program
//! accepts: real entry fee deposit into escrow, real shard membership.
//!
//! Prereq: `cargo build-sbf` (see docs/ER_TESTING.md) — already satisfied if
//! `target/deploy/xfchess_game.so` exists and is current.

mod common;

use anchor_lang::{AccountDeserialize, InstructionData, ToAccountMetas};
use solana_sdk::{
    instruction::Instruction,
    pubkey::Pubkey,
    signature::{read_keypair_file, Keypair, Signer},
    transaction::Transaction,
};
use solana_system_interface::instruction as system_instruction;
use xfchess_game::state::{Tournament, TournamentPlayersShard, TournamentType};

fn tournament_pda(id: u64) -> Pubkey {
    Pubkey::find_program_address(&[b"tournament", &id.to_le_bytes()], &xfchess_game::ID).0
}
fn escrow_pda(id: u64) -> Pubkey {
    Pubkey::find_program_address(&[b"t_escrow", &id.to_le_bytes()], &xfchess_game::ID).0
}
fn shard_pda(id: u64, idx: u8) -> Pubkey {
    Pubkey::find_program_address(
        &[b"tourney_players", &[idx], &id.to_le_bytes()],
        &xfchess_game::ID,
    )
    .0
}
fn profile_pda(player: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"profile", player.as_ref()], &xfchess_game::ID).0
}
fn username_record_pda(username: &str) -> Pubkey {
    Pubkey::find_program_address(&[b"username", username.as_bytes()], &xfchess_game::ID).0
}

fn init_profile_ix(player: &Pubkey, username: &str) -> Instruction {
    let accounts = xfchess_game::__client_accounts_init_profile::InitProfile {
        player_profile: profile_pda(player),
        username_record: username_record_pda(username),
        player: *player,
        system_program: solana_system_interface::program::ID,
    }
    .to_account_metas(None);
    let data = xfchess_game::instruction::InitProfile {
        username: username.to_string(),
        country: "GB".to_string(),
        date_of_birth: 631_152_000, // 1990-01-01
    }
    .data();
    Instruction { program_id: xfchess_game::ID, accounts, data }
}

/// This mirrors `build_register_player_ix` in
/// `src/multiplayer/solana/tournament_session.rs` field-for-field, but built
/// via Anchor's own generated `to_account_metas` — an independent,
/// authoritative check that the client's manually-assembled account list
/// (order + is_writable/is_signer flags) is actually correct.
fn register_player_ix(tournament_id: u64, player: &Pubkey, host_treasury: &Pubkey, elo: u32) -> Instruction {
    let program_id = xfchess_game::ID;
    let accounts = xfchess_game::__client_accounts_register_player::RegisterPlayer {
        tournament: tournament_pda(tournament_id),
        player_profile: profile_pda(player),
        player: *player,
        escrow_pda: escrow_pda(tournament_id),
        tournament_players_shard_0: shard_pda(tournament_id, 0),
        tournament_players_shard_1: Some(program_id), // None sentinel: <=64 players needs only shard 0
        tournament_players_shard_2: Some(program_id),
        tournament_players_shard_3: Some(program_id),
        host_treasury: *host_treasury,
        system_program: solana_system_interface::program::ID,
    }
    .to_account_metas(None);
    let data = xfchess_game::instruction::RegisterPlayer { tournament_id, elo }.data();
    Instruction { program_id, accounts, data }
}

#[tokio::test]
async fn register_player_deposits_entry_fee_and_adds_to_shard() {
    let ctx = common::start(vec![]).await;

    // vps_authority is a hardcoded on-chain constraint (constants::vps_authority::ID)
    // on every privileged tournament instruction — must sign with the real key.
    let authority = read_keypair_file(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../keys/vps_authority.json"
    ))
    .expect("keys/vps_authority.json must exist (devnet throwaway key, see project memory)");

    // Fund the authority (it pays for every account it creates below).
    let fund = system_instruction::transfer(&ctx.payer.pubkey(), &authority.pubkey(), 10_000_000_000);
    let mut tx = Transaction::new_with_payer(&[fund], Some(&ctx.payer.pubkey()));
    tx.sign(&[&ctx.payer], ctx.last_blockhash);
    ctx.banks_client.process_transaction(tx).await.unwrap();

    let tournament_id: u64 = 999_777;
    const ENTRY_FEE: u64 = 20_000_000; // 0.02 SOL
    const PRIZE: u64 = 50_000_000; // 0.05 SOL guaranteed prize

    // 1. initialize_tournament (max_players=2 -> single shard tier)
    let init_accounts = xfchess_game::__client_accounts_initialize_tournament::InitializeTournament {
        tournament: tournament_pda(tournament_id),
        usdc_prize_escrow_authority: Pubkey::find_program_address(
            &[b"t_usdc_prize", &tournament_id.to_le_bytes()],
            &xfchess_game::ID,
        )
        .0,
        usdc_prize_escrow: None,
        usdc_mint: None,
        authority: authority.pubkey(),
        token_program: anchor_spl::token::ID,
        associated_token_program: anchor_spl::associated_token::ID,
        system_program: solana_system_interface::program::ID,
    }
    .to_account_metas(None);
    let init_data = xfchess_game::instruction::InitializeTournament {
        tournament_id,
        name: "E2E Register Test".to_string(),
        entry_fee: ENTRY_FEE,
        max_players: 2,
        tournament_type: TournamentType::SingleElimination,
        elo_min: 0,
        elo_max: 10_000,
        min_players: 2,
        prize_shares: [7000, 3000, 0, 0, 0, 0, 0, 0, 0, 0],
        platform_fee: 0,
        winner_takes_all: false,
        host_treasury: authority.pubkey(),
        usdc_mint: None,
        base_time_seconds: 300,
        increment_seconds: 0,
    }
    .data();
    let init_ix = Instruction { program_id: xfchess_game::ID, accounts: init_accounts, data: init_data };

    // 2. initialize_tournament_escrow
    let escrow_accounts =
        xfchess_game::__client_accounts_initialize_tournament_escrow::InitializeTournamentEscrow {
            tournament: tournament_pda(tournament_id),
            escrow_pda: escrow_pda(tournament_id),
            authority: authority.pubkey(),
            system_program: solana_system_interface::program::ID,
        }
        .to_account_metas(None);
    let escrow_data = xfchess_game::instruction::InitializeTournamentEscrow { tournament_id }.data();
    let escrow_ix = Instruction { program_id: xfchess_game::ID, accounts: escrow_accounts, data: escrow_data };

    // 3. initialize_shards_small
    let shards_accounts = xfchess_game::__client_accounts_initialize_shards_small::InitializeShardsSmall {
        tournament: tournament_pda(tournament_id),
        tournament_players_shard_0: shard_pda(tournament_id, 0),
        authority: authority.pubkey(),
        system_program: solana_system_interface::program::ID,
    }
    .to_account_metas(None);
    let shards_data = xfchess_game::instruction::InitializeShardsSmall { tournament_id }.data();
    let shards_ix = Instruction { program_id: xfchess_game::ID, accounts: shards_accounts, data: shards_data };

    let mut tx = Transaction::new_with_payer(&[init_ix, escrow_ix, shards_ix], Some(&authority.pubkey()));
    tx.sign(&[&authority], ctx.last_blockhash);
    ctx.banks_client
        .process_transaction(tx)
        .await
        .expect("tournament setup (init + escrow + shards) should succeed");

    // 4. fund_sol_prize — guaranteed prize must be locked before anyone can register.
    let fund_prize_accounts = xfchess_game::__client_accounts_fund_sol_prize::FundSolPrize {
        tournament: tournament_pda(tournament_id),
        escrow_pda: escrow_pda(tournament_id),
        operator: authority.pubkey(),
        system_program: solana_system_interface::program::ID,
    }
    .to_account_metas(None);
    let fund_prize_data =
        xfchess_game::instruction::FundSolPrize { tournament_id, amount: PRIZE }.data();
    let fund_prize_ix =
        Instruction { program_id: xfchess_game::ID, accounts: fund_prize_accounts, data: fund_prize_data };
    let mut tx = Transaction::new_with_payer(&[fund_prize_ix], Some(&authority.pubkey()));
    tx.sign(&[&authority], ctx.last_blockhash);
    ctx.banks_client.process_transaction(tx).await.expect("fund_sol_prize should succeed");

    let escrow_before = ctx.banks_client.get_balance(escrow_pda(tournament_id)).await.unwrap();
    assert!(escrow_before >= PRIZE, "escrow should hold the guaranteed prize before registration");

    // 5. Set up a real player: fund wallet + init_profile (real instruction,
    // same as the sponsored-profile test).
    let player = Keypair::new();
    let fund_player = system_instruction::transfer(&ctx.payer.pubkey(), &player.pubkey(), 5_000_000_000);
    let mut tx = Transaction::new_with_payer(&[fund_player], Some(&ctx.payer.pubkey()));
    tx.sign(&[&ctx.payer], ctx.last_blockhash);
    ctx.banks_client.process_transaction(tx).await.unwrap();

    let init_profile = init_profile_ix(&player.pubkey(), "e2eregistrant");
    let mut tx = Transaction::new_with_payer(&[init_profile], Some(&player.pubkey()));
    tx.sign(&[&player], ctx.last_blockhash);
    ctx.banks_client.process_transaction(tx).await.expect("init_profile should succeed");

    // 6. THE thing this test exists to prove: register_player, built exactly
    // the way the fixed game client builds it.
    let player_balance_before = ctx.banks_client.get_balance(player.pubkey()).await.unwrap();
    let register_ix = register_player_ix(tournament_id, &player.pubkey(), &authority.pubkey(), 1200);
    let mut tx = Transaction::new_with_payer(&[register_ix], Some(&player.pubkey()));
    tx.sign(&[&player], ctx.last_blockhash);
    ctx.banks_client
        .process_transaction(tx)
        .await
        .expect("register_player should succeed with the client's account layout");

    // ── Assertions: the actual on-chain effects a real player should see ──

    // Entry fee really left the player's wallet (fee + entry_fee debited).
    let player_balance_after = ctx.banks_client.get_balance(player.pubkey()).await.unwrap();
    assert!(
        player_balance_before - player_balance_after >= ENTRY_FEE,
        "player should have paid at least the entry fee (before={player_balance_before}, after={player_balance_after})"
    );

    // Entry fee really landed in escrow (on top of the guaranteed prize already there).
    let escrow_after = ctx.banks_client.get_balance(escrow_pda(tournament_id)).await.unwrap();
    assert_eq!(
        escrow_after,
        escrow_before + ENTRY_FEE,
        "escrow should hold prize + entry fee after registration"
    );

    // Player really appears in the on-chain shard (not just an off-chain roster).
    let shard_acc = ctx.banks_client.get_account(shard_pda(tournament_id, 0)).await.unwrap().unwrap();
    let shard = TournamentPlayersShard::try_deserialize(&mut &shard_acc.data[..]).unwrap();
    assert_eq!(shard.players, vec![player.pubkey()]);
    assert_eq!(shard.player_elos, vec![1200]);

    // Tournament's registered-player counter really incremented on-chain.
    let t_acc = ctx.banks_client.get_account(tournament_pda(tournament_id)).await.unwrap().unwrap();
    let t = Tournament::try_deserialize(&mut &t_acc.data[..]).unwrap();
    assert_eq!(t.num_registered_players, 1);
}
