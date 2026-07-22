//! Integration tests for permissionless Swiss round advancement.
//!
//! Runs the real compiled program (`target/deploy/xfchess_game.so`) in-process
//! via `solana-program-test`, seeding a small (2-player, 1-round) Swiss
//! tournament directly. Proves the actual persistency property: a round can
//! advance using only the two players' own signatures on `record_swiss_result`
//! plus an `advance_round` crank from an arbitrary third party — no
//! tournament-authority signer, no backend process, involved at all.
//!
//! Build the `.so` first with:
//!   cargo build-sbf --manifest-path programs/xfchess-game/Cargo.toml

use anchor_lang::{AccountSerialize, InstructionData, Space, ToAccountMetas};
use solana_program_test::{
    BanksClientError, ProgramTest, ProgramTestBanksClientExt, ProgramTestContext,
};
use solana_sdk::{
    account::Account,
    instruction::{Instruction, InstructionError},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::{Transaction, TransactionError},
};
use xfchess_game::errors::GameErrorCode;
use xfchess_game::state::{
    get_default_prize_shares, PayoutType, SwissStanding, Tournament, TournamentPlayersShard,
    TournamentStatus, TournamentType,
};
use xfchess_game::tournament_ix::matches::SwissMatchResult;

const PROGRAM: &str = "xfchess_game";
const TOURNAMENT_ID: u64 = 1;

fn ec(e: GameErrorCode) -> u32 {
    6000 + e as u32
}

fn tournament_pda(id: u64) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"tournament", &id.to_le_bytes()], &xfchess_game::ID)
}

fn shard_pda(id: u64, shard: u8) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"tourney_players", &[shard], &id.to_le_bytes()],
        &xfchess_game::ID,
    )
}

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

/// A 2-player, 1-round Swiss tournament, already `Active` with round 0 in
/// progress and both players registered in shard 0.
fn tournament(white: Pubkey, bump: u8) -> Tournament {
    Tournament {
        tournament_id: TOURNAMENT_ID,
        authority: Pubkey::new_unique(),
        name: String::new(),
        entry_fee: 0,
        platform_fee: 0,
        prize_pool: 0,
        max_players: 2,
        player_count: 2,
        num_registered_players: 2,
        status: TournamentStatus::Active,
        start_time: None,
        end_time: None,
        fees_advanced: 0,
        fee_payer: white,
        tournament_type: TournamentType::Swiss { rounds: 1 },
        current_round: 0,
        total_rounds: 1,
        total_matches: 1,
        final_match_index: 0,
        elo_min: 0,
        elo_max: 4000,
        min_players: 2,
        winner: None,
        second_place: None,
        third_place: None,
        fourth_place: None,
        fifth_place: None,
        sixth_place: None,
        seventh_place: None,
        eighth_place: None,
        ninth_place: None,
        tenth_place: None,
        prize_shares: get_default_prize_shares(2, false),
        created_at: 0,
        started_at: Some(0),
        completed_at: None,
        bump,
        prizes_claimed: 0,
        platform_fee_pool: 0,
        usdc_prize_mint: None,
        usdc_prize_pool: 0,
        usdc_prize_funded: false,
        host_treasury: Pubkey::new_unique(),
        prize_token_mint: None,
        payout_type: PayoutType::LumpSum,
        vesting_params: None,
        base_time_seconds: 0,
        increment_seconds: 0,
        winner_takes_all: false,
        round_boards_reported: [0u8; 16],
    }
}

fn shard0(white: Pubkey, black: Pubkey) -> TournamentPlayersShard {
    TournamentPlayersShard {
        tournament_id: TOURNAMENT_ID,
        shard_id: 0,
        players: vec![white, black],
        player_elos: vec![1200, 1200],
        swiss_standings: vec![
            SwissStanding {
                player: white,
                score: 0,
                buchholz: 0,
                sonneborn: 0,
                color_balance: 0,
            },
            SwissStanding {
                player: black,
                score: 0,
                buchholz: 0,
                sonneborn: 0,
                color_balance: 0,
            },
        ],
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

fn record_swiss_result_ix(round: u8, board: u16, player: Pubkey, opponent: Pubkey) -> Instruction {
    let accounts = xfchess_game::__client_accounts_record_swiss_result::RecordSwissResult {
        tournament: tournament_pda(TOURNAMENT_ID).0,
        tournament_players_shard_0: shard_pda(TOURNAMENT_ID, 0).0,
        tournament_players_shard_1: None,
        tournament_players_shard_2: None,
        tournament_players_shard_3: None,
        player,
        opponent,
        system_program: solana_system_interface::program::ID,
    }
    .to_account_metas(None);
    let data = xfchess_game::instruction::RecordSwissResult {
        tournament_id: TOURNAMENT_ID,
        round,
        board,
        result: SwissMatchResult::Win,
    }
    .data();
    Instruction {
        program_id: xfchess_game::ID,
        accounts,
        data,
    }
}

fn advance_round_ix(cranker: Pubkey) -> Instruction {
    let accounts = xfchess_game::__client_accounts_advance_round::AdvanceRound {
        tournament: tournament_pda(TOURNAMENT_ID).0,
        cranker,
    }
    .to_account_metas(None);
    let data = xfchess_game::instruction::AdvanceRound {
        tournament_id: TOURNAMENT_ID,
    }
    .data();
    Instruction {
        program_id: xfchess_game::ID,
        accounts,
        data,
    }
}

/// Fetches a fresh blockhash before every send (rather than reusing
/// `ctx.last_blockhash`) — this test sends several transactions back to
/// back, and an identical instruction/signer set on a stale blockhash would
/// produce an identical signature, which the bank treats as "already
/// processed" and silently no-ops instead of re-executing the handler.
async fn send(
    ctx: &mut ProgramTestContext,
    ix: Instruction,
    extra: &[&Keypair],
) -> Result<(), TransactionError> {
    let blockhash = ctx
        .banks_client
        .get_new_latest_blockhash(&ctx.last_blockhash)
        .await
        .expect("failed to get a fresh blockhash");
    ctx.last_blockhash = blockhash;
    let payer_pk = ctx.payer.pubkey();
    let mut signers: Vec<&Keypair> = vec![&ctx.payer];
    signers.extend_from_slice(extra);
    let mut tx = Transaction::new_with_payer(&[ix], Some(&payer_pk));
    tx.sign(&signers, blockhash);
    ctx.banks_client
        .process_transaction(tx)
        .await
        .map_err(transaction_error)
}

fn transaction_error(e: BanksClientError) -> TransactionError {
    match e {
        BanksClientError::TransactionError(te) => te,
        BanksClientError::SimulationError { err, .. } => err,
        other => panic!("unexpected banks error: {other:?}"),
    }
}

fn custom_code(err: &TransactionError) -> Option<u32> {
    match err {
        TransactionError::InstructionError(_, InstructionError::Custom(c)) => Some(*c),
        _ => None,
    }
}

async fn fetch_tournament(ctx: &mut ProgramTestContext) -> Tournament {
    use anchor_lang::AccountDeserialize;
    let acc = ctx
        .banks_client
        .get_account(tournament_pda(TOURNAMENT_ID).0)
        .await
        .unwrap()
        .expect("tournament account missing");
    Tournament::try_deserialize(&mut &acc.data[..]).unwrap()
}

/// The property this whole feature exists for: with only the two players'
/// own signatures (no tournament authority, no backend process) the round
/// can be recorded and advanced by a completely unrelated third-party cranker.
#[tokio::test]
async fn round_advances_from_player_signatures_and_a_third_party_crank() {
    let white = Keypair::new();
    let black = Keypair::new();
    let cranker = Keypair::new();
    let (_, t_bump) = tournament_pda(TOURNAMENT_ID);

    let mut ctx = start(vec![
        (
            tournament_pda(TOURNAMENT_ID).0,
            serialize_padded(
                &tournament(white.pubkey(), t_bump),
                8 + Tournament::INIT_SPACE,
            ),
        ),
        (
            shard_pda(TOURNAMENT_ID, 0).0,
            serialize_padded(
                &shard0(white.pubkey(), black.pubkey()),
                8 + TournamentPlayersShard::space_for(),
            ),
        ),
    ])
    .await;

    // Advancing before the (only) board has reported must fail.
    let err = send(&mut ctx, advance_round_ix(cranker.pubkey()), &[&cranker])
        .await
        .unwrap_err();
    assert_eq!(
        custom_code(&err),
        Some(ec(GameErrorCode::TournamentRoundIncomplete))
    );

    // The white player signs their own result — no authority/backend involved.
    send(
        &mut ctx,
        record_swiss_result_ix(0, 0, white.pubkey(), black.pubkey()),
        &[&white],
    )
    .await
    .expect("record_swiss_result should succeed");

    // Recording the same board again must be rejected (idempotency guard).
    let err = send(
        &mut ctx,
        record_swiss_result_ix(0, 0, white.pubkey(), black.pubkey()),
        &[&white],
    )
    .await
    .unwrap_err();
    assert_eq!(
        custom_code(&err),
        Some(ec(GameErrorCode::BoardAlreadyRecorded))
    );

    // Now a completely unrelated party can crank the round forward.
    send(&mut ctx, advance_round_ix(cranker.pubkey()), &[&cranker])
        .await
        .expect("advance_round should succeed once the board is in");

    let t = fetch_tournament(&mut ctx).await;
    assert_eq!(t.current_round, 1);
    assert_eq!(t.round_boards_reported, [0u8; 16]);

    // Nothing left to advance to (round == total_rounds).
    let err = send(&mut ctx, advance_round_ix(cranker.pubkey()), &[&cranker])
        .await
        .unwrap_err();
    assert_eq!(
        custom_code(&err),
        Some(ec(GameErrorCode::InvalidGameStatus))
    );
}
