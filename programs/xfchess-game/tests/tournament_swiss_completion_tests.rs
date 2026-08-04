//! Integration tests for `complete_swiss_tournament`.
//!
//! Runs the real compiled program (`target/deploy/xfchess_game.so`) in-process
//! via `solana-program-test`. Proves the gap flagged in `advance_round.rs`'s
//! doc comment is actually closed: once every round has been played and
//! `advance_round` has pushed `current_round` to `total_rounds`, a completely
//! unrelated third party can crank `complete_swiss_tournament` to sort final
//! standings and mark the tournament `Completed` — no tournament-authority
//! signer, no backend process, involved.
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

/// A 4-player, 2-round Swiss tournament, `Active`, with `current_round`
/// already at `total_rounds` (as if both rounds had been played and
/// `advance_round` cranked twice) so `complete_swiss_tournament` is eligible.
fn tournament(fee_payer: Pubkey, bump: u8, current_round: u8) -> Tournament {
    Tournament {
        tournament_id: TOURNAMENT_ID,
        authority: Pubkey::new_unique(),
        name: String::new(),
        entry_fee: 0,
        platform_fee: 0,
        prize_pool: 1_000_000,
        max_players: 4,
        player_count: 4,
        num_registered_players: 4,
        status: TournamentStatus::Active,
        start_time: None,
        end_time: None,
        fees_advanced: 0,
        fee_payer,
        tournament_type: TournamentType::Swiss { rounds: 2 },
        current_round,
        total_rounds: 2,
        total_matches: 4,
        final_match_index: 3,
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
        prize_shares: get_default_prize_shares(4, false),
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

/// Four players with distinct scores/tiebreakers so sort order is unambiguous:
/// p1 wins outright on score, p2 beats p3 on Buchholz despite equal score,
/// p4 is last.
fn shard0(p1: Pubkey, p2: Pubkey, p3: Pubkey, p4: Pubkey) -> TournamentPlayersShard {
    TournamentPlayersShard {
        tournament_id: TOURNAMENT_ID,
        shard_id: 0,
        players: vec![p1, p2, p3, p4],
        player_elos: vec![1200, 1200, 1200, 1200],
        swiss_standings: vec![
            SwissStanding {
                player: p1,
                score: 4,
                buchholz: 3,
                sonneborn: 3,
                color_balance: 0,
            },
            SwissStanding {
                player: p2,
                score: 2,
                buchholz: 5,
                sonneborn: 2,
                color_balance: 0,
            },
            SwissStanding {
                player: p3,
                score: 2,
                buchholz: 3,
                sonneborn: 1,
                color_balance: 0,
            },
            SwissStanding {
                player: p4,
                score: 0,
                buchholz: 2,
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

fn complete_swiss_tournament_ix(cranker: Pubkey) -> Instruction {
    let accounts =
        xfchess_game::__client_accounts_complete_swiss_tournament::CompleteSwissTournament {
            tournament: tournament_pda(TOURNAMENT_ID).0,
            tournament_players_shard_0: shard_pda(TOURNAMENT_ID, 0).0,
            tournament_players_shard_1: None,
            tournament_players_shard_2: None,
            tournament_players_shard_3: None,
            cranker,
        }
        .to_account_metas(None);
    let data = xfchess_game::instruction::CompleteSwissTournament {
        tournament_id: TOURNAMENT_ID,
    }
    .data();
    Instruction {
        program_id: xfchess_game::ID,
        accounts,
        data,
    }
}

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

async fn setup(
    current_round: u8,
    p1: Pubkey,
    p2: Pubkey,
    p3: Pubkey,
    p4: Pubkey,
) -> ProgramTestContext {
    let (_, t_bump) = tournament_pda(TOURNAMENT_ID);
    start(vec![
        (
            tournament_pda(TOURNAMENT_ID).0,
            serialize_padded(
                &tournament(p1, t_bump, current_round),
                8 + Tournament::INIT_SPACE,
            ),
        ),
        (
            shard_pda(TOURNAMENT_ID, 0).0,
            serialize_padded(
                &shard0(p1, p2, p3, p4),
                8 + TournamentPlayersShard::space_for(),
            ),
        ),
    ])
    .await
}

/// The property this feature exists for: a Swiss tournament with all rounds
/// played can be finalized by a completely unrelated third-party cranker, and
/// the resulting placements match the on-chain standings' score/Buchholz/
/// Sonneborn ordering.
#[tokio::test]
async fn completes_and_ranks_by_score_then_tiebreakers() {
    let p1 = Keypair::new();
    let p2 = Keypair::new();
    let p3 = Keypair::new();
    let p4 = Keypair::new();
    let cranker = Keypair::new();

    let mut ctx = setup(
        2, // current_round == total_rounds: both rounds already advanced
        p1.pubkey(),
        p2.pubkey(),
        p3.pubkey(),
        p4.pubkey(),
    )
    .await;

    send(
        &mut ctx,
        complete_swiss_tournament_ix(cranker.pubkey()),
        &[&cranker],
    )
    .await
    .expect("complete_swiss_tournament should succeed once all rounds are in");

    let t = fetch_tournament(&mut ctx).await;
    assert_eq!(t.status, TournamentStatus::Completed);
    assert!(t.completed_at.is_some());
    // p1: score 4 (highest) -> 1st.
    assert_eq!(t.winner, Some(p1.pubkey()));
    // p2 and p3 tie on score (2) but p2 has the higher Buchholz (5 vs 3) -> 2nd.
    assert_eq!(t.second_place, Some(p2.pubkey()));
    assert_eq!(t.third_place, Some(p3.pubkey()));
    // p4: score 0 (lowest) -> 4th.
    assert_eq!(t.fourth_place, Some(p4.pubkey()));
    assert_eq!(t.fifth_place, None);
}

/// Calling before the last round has been advanced must fail — otherwise a
/// tournament could be paid out and closed while rounds are still in play.
#[tokio::test]
async fn rejects_completion_before_final_round_advances() {
    let p1 = Keypair::new();
    let p2 = Keypair::new();
    let p3 = Keypair::new();
    let p4 = Keypair::new();
    let cranker = Keypair::new();

    let mut ctx = setup(
        1, // only round 0 of 2 has been advanced past
        p1.pubkey(),
        p2.pubkey(),
        p3.pubkey(),
        p4.pubkey(),
    )
    .await;

    let err = send(
        &mut ctx,
        complete_swiss_tournament_ix(cranker.pubkey()),
        &[&cranker],
    )
    .await
    .unwrap_err();
    assert_eq!(
        custom_code(&err),
        Some(ec(GameErrorCode::SwissTournamentNotFinished))
    );
}
