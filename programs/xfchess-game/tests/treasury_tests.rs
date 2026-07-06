//! Integration tests for the treasury-withdrawal and close_tournament fixes.
//!
//! Runs the real compiled program (`target/deploy/xfchess_game.so`) in-process
//! via `solana-program-test`, seeding account state directly. Build the `.so`
//! first with `cargo build-sbf --manifest-path programs/xfchess-game/Cargo.toml`.
//!
//! Coverage:
//!   withdraw_treasury — happy path, rent floor, wrong signer, zero amount
//!   close_tournament  — Active blocked, unpaid winner blocked, all-claimed OK

use anchor_lang::{AccountSerialize, InstructionData, Space, ToAccountMetas};
use solana_program_test::{BanksClientError, ProgramTest, ProgramTestContext};
use solana_sdk::{
    account::Account,
    instruction::{Instruction, InstructionError},
    pubkey::Pubkey,
    signature::{read_keypair_file, Keypair, Signer},
    system_program,
    transaction::{Transaction, TransactionError},
};
use xfchess_game::errors::GameErrorCode;
use xfchess_game::state::{PayoutType, Tournament, TournamentStatus, TournamentType};

const PROGRAM: &str = "xfchess_game";

fn ec(e: GameErrorCode) -> u32 {
    6000 + e as u32
}

fn treasury_vault_pda() -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"treasury_vault"], &xfchess_game::ID)
}

fn tournament_pda(id: u64) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"tournament", &id.to_le_bytes()], &xfchess_game::ID)
}

fn tournament_escrow_pda(id: u64) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"t_escrow", &id.to_le_bytes()], &xfchess_game::ID)
}

/// System-owned account holding `lamports` and no data (wallet / bare PDA vault).
fn system_account(lamports: u64) -> Account {
    Account {
        lamports,
        data: vec![],
        owner: system_program::id(),
        executable: false,
        rent_epoch: 0,
    }
}

/// Program-owned account with `lamports` and exactly `data_len` zero bytes.
fn program_owned(lamports: u64, data_len: usize) -> Account {
    Account {
        lamports,
        data: vec![0u8; data_len],
        owner: xfchess_game::ID,
        executable: false,
        rent_epoch: 0,
    }
}

/// Serialize an anchor account and pad to its full on-chain allocation.
fn serialize_padded<T: AccountSerialize>(value: &T, space: usize) -> Account {
    let mut data = Vec::with_capacity(space);
    value.try_serialize(&mut data).unwrap();
    if data.len() < space {
        data.resize(space, 0);
    }
    Account {
        lamports: 10_000_000,
        data,
        owner: xfchess_game::ID,
        executable: false,
        rent_epoch: 0,
    }
}

async fn start(accounts: Vec<(Pubkey, Account)>) -> ProgramTestContext {
    std::env::set_var(
        "SBF_OUT_DIR",
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/deploy"),
    );
    let mut pt = ProgramTest::new(PROGRAM, xfchess_game::ID, None);
    for (key, account) in accounts {
        pt.add_account(key, account);
    }
    pt.start_with_context().await
}

async fn send(
    ctx: &mut ProgramTestContext,
    ix: Instruction,
    extra: &[&Keypair],
) -> Result<(), TransactionError> {
    let blockhash = ctx.last_blockhash;
    let payer_pk = ctx.payer.pubkey();
    let mut signers: Vec<&Keypair> = vec![&ctx.payer];
    signers.extend_from_slice(extra);
    let mut tx = Transaction::new_with_payer(&[ix], Some(&payer_pk));
    tx.sign(&signers, blockhash);
    ctx.banks_client
        .process_transaction(tx)
        .await
        .map_err(|e| match e {
            BanksClientError::TransactionError(te) => te,
            BanksClientError::SimulationError { err, .. } => err,
            other => panic!("unexpected banks error: {other:?}"),
        })
}

fn custom_code(err: &TransactionError) -> Option<u32> {
    match err {
        TransactionError::InstructionError(_, InstructionError::Custom(c)) => Some(*c),
        _ => None,
    }
}

async fn lamports_of(ctx: &mut ProgramTestContext, key: Pubkey) -> u64 {
    ctx.banks_client
        .get_account(key)
        .await
        .unwrap()
        .map(|a| a.lamports)
        .unwrap_or(0)
}

// ── withdraw_treasury ───────────────────────────────────────────────────────

fn withdraw_ix(authority: Pubkey, destination: Pubkey, amount: u64) -> Instruction {
    let accounts = xfchess_game::__client_accounts_withdraw_treasury::WithdrawTreasury {
        treasury_vault: treasury_vault_pda().0,
        authority,
        destination,
        system_program: system_program::id(),
    }
    .to_account_metas(None);
    let data = xfchess_game::instruction::WithdrawTreasury { amount }.data();
    Instruction {
        program_id: xfchess_game::ID,
        accounts,
        data,
    }
}

/// Load the real treasury authority keypair from the gitignored keyfile.
/// Returns None (test skips the signing path) when it isn't present (e.g. CI).
fn treasury_authority_keypair() -> Option<Keypair> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../keys/treasury_authority.json"
    );
    read_keypair_file(path).ok()
}

#[tokio::test]
async fn withdraw_treasury_happy_path() {
    let Some(authority) = treasury_authority_keypair() else {
        eprintln!("skip: keys/treasury_authority.json not present");
        return;
    };
    let vault = treasury_vault_pda().0;
    let dest = Keypair::new().pubkey();

    let mut ctx = start(vec![
        (vault, system_account(1_000_000_000)),  // 1 SOL accrued fees
        (authority.pubkey(), system_account(0)), // signer, no lamports needed
        (dest, system_account(0)),
    ])
    .await;

    let before = lamports_of(&mut ctx, dest).await;
    send(
        &mut ctx,
        withdraw_ix(authority.pubkey(), dest, 400_000_000),
        &[&authority],
    )
    .await
    .expect("withdraw should succeed");

    assert_eq!(lamports_of(&mut ctx, dest).await, before + 400_000_000);
    assert_eq!(lamports_of(&mut ctx, vault).await, 600_000_000);
}

#[tokio::test]
async fn withdraw_treasury_rent_floor_rejected() {
    let Some(authority) = treasury_authority_keypair() else {
        eprintln!("skip: keys/treasury_authority.json not present");
        return;
    };
    let vault = treasury_vault_pda().0;
    let dest = Keypair::new().pubkey();

    // Vault holds slightly above the rent-exempt min for 0 data; draining all of
    // it must be rejected to keep the vault alive.
    let mut ctx = start(vec![
        (vault, system_account(1_000_000)),
        (authority.pubkey(), system_account(0)),
        (dest, system_account(0)),
    ])
    .await;

    let err = send(
        &mut ctx,
        withdraw_ix(authority.pubkey(), dest, 1_000_000),
        &[&authority],
    )
    .await
    .unwrap_err();
    assert_eq!(
        custom_code(&err),
        Some(ec(GameErrorCode::InsufficientFunds))
    );
}

#[tokio::test]
async fn withdraw_treasury_wrong_signer_rejected() {
    let vault = treasury_vault_pda().0;
    let attacker = Keypair::new();
    let dest = Keypair::new().pubkey();

    let mut ctx = start(vec![
        (vault, system_account(1_000_000_000)),
        (attacker.pubkey(), system_account(0)),
        (dest, system_account(0)),
    ])
    .await;

    let err = send(
        &mut ctx,
        withdraw_ix(attacker.pubkey(), dest, 100),
        &[&attacker],
    )
    .await
    .unwrap_err();
    // Anchor `address = …` violation maps to the mapped error code.
    assert_eq!(
        custom_code(&err),
        Some(ec(GameErrorCode::UnauthorizedAccess))
    );
}

#[tokio::test]
async fn withdraw_treasury_zero_amount_rejected() {
    let Some(authority) = treasury_authority_keypair() else {
        eprintln!("skip: keys/treasury_authority.json not present");
        return;
    };
    let vault = treasury_vault_pda().0;
    let dest = Keypair::new().pubkey();

    let mut ctx = start(vec![
        (vault, system_account(1_000_000_000)),
        (authority.pubkey(), system_account(0)),
        (dest, system_account(0)),
    ])
    .await;

    let err = send(
        &mut ctx,
        withdraw_ix(authority.pubkey(), dest, 0),
        &[&authority],
    )
    .await
    .unwrap_err();
    assert_eq!(custom_code(&err), Some(ec(GameErrorCode::InvalidArgument)));
}

// ── close_tournament ────────────────────────────────────────────────────────

/// Build a Tournament with the fields the close path reads; everything else default.
#[allow(clippy::too_many_arguments)]
fn tournament(
    id: u64,
    authority: Pubkey,
    status: TournamentStatus,
    winner: Option<Pubkey>,
    prize_shares: [u16; 10],
    prizes_claimed: u16,
    prize_pool: u64,
    bump: u8,
) -> Tournament {
    Tournament {
        tournament_id: id,
        authority,
        name: String::new(),
        entry_fee: 0,
        platform_fee: 0,
        prize_pool,
        max_players: 8,
        player_count: 0,
        num_registered_players: 0,
        status,
        start_time: None,
        end_time: None,
        fees_advanced: 0,
        fee_payer: authority,
        tournament_type: TournamentType::SingleElimination,
        current_round: 0,
        total_rounds: 0,
        total_matches: 7,
        final_match_index: 6,
        elo_min: 0,
        elo_max: 4000,
        min_players: 2,
        winner,
        second_place: None,
        third_place: None,
        fourth_place: None,
        fifth_place: None,
        sixth_place: None,
        seventh_place: None,
        eighth_place: None,
        ninth_place: None,
        tenth_place: None,
        prize_shares,
        created_at: 0,
        started_at: None,
        completed_at: None,
        bump,
        prizes_claimed,
        platform_fee_pool: 0,
        usdc_prize_mint: None,
        usdc_prize_pool: 0,
        usdc_prize_funded: false,
        host_treasury: authority,
        prize_token_mint: None,
        payout_type: PayoutType::LumpSum,
        vesting_params: None,
        base_time_seconds: 0,
        increment_seconds: 0,
        winner_takes_all: false,
    }
}

fn close_ix(id: u64, authority: Pubkey) -> Instruction {
    let accounts = xfchess_game::__client_accounts_close_tournament::CloseTournament {
        tournament: tournament_pda(id).0,
        prize_escrow_pda: tournament_escrow_pda(id).0,
        treasury_vault: treasury_vault_pda().0,
        system_program: system_program::id(),
        authority,
    }
    .to_account_metas(None);
    let data = xfchess_game::instruction::CloseTournament { tournament_id: id }.data();
    Instruction {
        program_id: xfchess_game::ID,
        accounts,
        data,
    }
}

/// Seed the accounts a close_tournament call touches.
fn close_accounts(id: u64, t: &Tournament, escrow_lamports: u64) -> Vec<(Pubkey, Account)> {
    vec![
        (
            tournament_pda(id).0,
            serialize_padded(t, 8 + Tournament::INIT_SPACE),
        ),
        (
            tournament_escrow_pda(id).0,
            program_owned(escrow_lamports, 8),
        ),
        (treasury_vault_pda().0, system_account(1_000_000)),
    ]
}

#[tokio::test]
async fn close_tournament_blocked_while_active() {
    let id = 1;
    let authority = Keypair::new();
    let bump = tournament_pda(id).1;
    let t = tournament(
        id,
        authority.pubkey(),
        TournamentStatus::Active,
        Some(Keypair::new().pubkey()),
        [6000, 3000, 1000, 0, 0, 0, 0, 0, 0, 0],
        0,
        1_000_000_000,
        bump,
    );
    let mut ctx = start(close_accounts(id, &t, 1_000_000_000)).await;

    let err = send(&mut ctx, close_ix(id, authority.pubkey()), &[&authority])
        .await
        .unwrap_err();
    assert_eq!(
        custom_code(&err),
        Some(ec(GameErrorCode::InvalidTournamentStatus))
    );
}

#[tokio::test]
async fn close_tournament_blocked_with_unpaid_winner() {
    let id = 2;
    let authority = Keypair::new();
    let bump = tournament_pda(id).1;
    // Completed, winner funded (share > 0), but claim bit NOT set → must block.
    let t = tournament(
        id,
        authority.pubkey(),
        TournamentStatus::Completed,
        Some(Keypair::new().pubkey()),
        [10000, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        0,
        1_000_000_000,
        bump,
    );
    let mut ctx = start(close_accounts(id, &t, 1_000_000_000)).await;

    let err = send(&mut ctx, close_ix(id, authority.pubkey()), &[&authority])
        .await
        .unwrap_err();
    assert_eq!(
        custom_code(&err),
        Some(ec(GameErrorCode::PrizesOutstanding))
    );
}

#[tokio::test]
async fn close_tournament_succeeds_when_all_claimed() {
    let id = 3;
    let authority = Keypair::new();
    let bump = tournament_pda(id).1;
    // Completed, winner's claim bit (bit 0) set → close allowed; residual dust
    // above rent-exempt is swept to the treasury vault.
    let t = tournament(
        id,
        authority.pubkey(),
        TournamentStatus::Completed,
        Some(Keypair::new().pubkey()),
        [10000, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        0b1, // bit 0 (winner) claimed
        1_000_000_000,
        bump,
    );
    let escrow = tournament_escrow_pda(id).0;
    let vault = treasury_vault_pda().0;
    let escrow_lamports = 5_000_000u64; // leftover dust in escrow
    let mut ctx = start(close_accounts(id, &t, escrow_lamports)).await;

    let vault_before = lamports_of(&mut ctx, vault).await;
    send(&mut ctx, close_ix(id, authority.pubkey()), &[&authority])
        .await
        .expect("close should succeed once winners are paid");

    // Escrow fully swept (account reclaimed) and its full balance moved to vault.
    let escrow_after = lamports_of(&mut ctx, escrow).await;
    let vault_after = lamports_of(&mut ctx, vault).await;
    assert_eq!(escrow_after, 0, "escrow should be fully reclaimed");
    assert_eq!(vault_after - vault_before, escrow_lamports);
}
