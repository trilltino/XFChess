#[path = "common/agave.rs"]
pub mod agave;
use anchor_lang::{AccountSerialize, InstructionData, Space, ToAccountMetas};
use anyhow::Context as _;
use crate::agave::{
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

fn serialize_padded<T: AccountSerialize>(value: &T, space: usize) -> anyhow::Result<Account> {
    Ok({
        let mut data = Vec::with_capacity(space);
        value
            .try_serialize(&mut data)
            .context("required operation value")?;
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
    })
}

fn tournament(authority: Pubkey, white: Pubkey, bump: u8) -> Tournament {
    Tournament {
        tournament_id: TOURNAMENT_ID,
        authority,
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

fn record_swiss_result_ix(
    authority: Pubkey,
    round: u8,
    board: u16,
    white: Pubkey,
    black: Pubkey,
    result: SwissMatchResult,
) -> Instruction {
    let accounts = xfchess_game::__client_accounts_record_swiss_result::RecordSwissResult {
        tournament: tournament_pda(TOURNAMENT_ID).0,
        tournament_players_shard_0: shard_pda(TOURNAMENT_ID, 0).0,
        tournament_players_shard_1: None,
        tournament_players_shard_2: None,
        tournament_players_shard_3: None,
        authority,
        white,
        black,
    }
    .to_account_metas(None);
    let data = xfchess_game::instruction::RecordSwissResult {
        tournament_id: TOURNAMENT_ID,
        round,
        board,
        result,
    }
    .data();
    Instruction {
        program_id: xfchess_game::ID,
        accounts,
        data,
    }
}

fn advance_round_ix(authority: Pubkey, boards_in_round: u16) -> Instruction {
    let accounts = xfchess_game::__client_accounts_advance_round::AdvanceRound {
        tournament: tournament_pda(TOURNAMENT_ID).0,
        authority,
    }
    .to_account_metas(None);
    let data = xfchess_game::instruction::AdvanceRound {
        tournament_id: TOURNAMENT_ID,
        boards_in_round,
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
) -> anyhow::Result<Result<(), TransactionError>> {
    Ok({
        let blockhash = ctx
            .banks_client
            .get_new_latest_blockhash(&ctx.last_blockhash)
            .await
            .context("failed to get a fresh blockhash")?;
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
    })
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

async fn fetch_tournament(ctx: &mut ProgramTestContext) -> anyhow::Result<Tournament> {
    Ok({
        use anchor_lang::AccountDeserialize;
        let acc = ctx
            .banks_client
            .get_account(tournament_pda(TOURNAMENT_ID).0)
            .await
            .context("required operation value")?
            .context("tournament account missing")?;
        Tournament::try_deserialize(&mut &acc.data[..]).context("required operation value")?
    })
}

async fn fetch_shard0(ctx: &mut ProgramTestContext) -> anyhow::Result<TournamentPlayersShard> {
    Ok({
        use anchor_lang::AccountDeserialize;
        let acc = ctx
            .banks_client
            .get_account(shard_pda(TOURNAMENT_ID, 0).0)
            .await
            .context("required operation value")?
            .context("shard account missing")?;
        TournamentPlayersShard::try_deserialize(&mut &acc.data[..])
            .context("required operation value")?
    })
}

async fn setup(
    authority: Pubkey,
    white: Pubkey,
    black: Pubkey,
) -> anyhow::Result<ProgramTestContext> {
    Ok({
        let (_, t_bump) = tournament_pda(TOURNAMENT_ID);
        start(vec![
            (
                tournament_pda(TOURNAMENT_ID).0,
                serialize_padded(
                    &tournament(authority, white, t_bump),
                    8 + Tournament::INIT_SPACE,
                )?,
            ),
            (
                shard_pda(TOURNAMENT_ID, 0).0,
                serialize_padded(
                    &shard0(white, black),
                    8 + TournamentPlayersShard::space_for(),
                )?,
            ),
        ])
        .await
    })
}

#[tokio::test]
async fn authority_records_the_board_then_advances_the_round() -> anyhow::Result<()> {
    let authority = Keypair::new();
    let white = Keypair::new();
    let black = Keypair::new();
    let mut ctx = setup(authority.pubkey(), white.pubkey(), black.pubkey()).await?;

    // Advancing before the (only) board has reported must fail.
    let err = send(
        &mut ctx,
        advance_round_ix(authority.pubkey(), 1),
        &[&authority],
    )
    .await?
    .err()
    .context("required operation value")?;
    assert_eq!(
        custom_code(&err),
        Some(ec(GameErrorCode::TournamentRoundIncomplete))
    );

    send(
        &mut ctx,
        record_swiss_result_ix(
            authority.pubkey(),
            0,
            0,
            white.pubkey(),
            black.pubkey(),
            SwissMatchResult::WhiteWin,
        ),
        &[&authority],
    )
    .await?
    .context("record_swiss_result should succeed")?;

    let shard = fetch_shard0(&mut ctx).await?;
    let white_standing = &shard.swiss_standings[0];
    let black_standing = &shard.swiss_standings[1];
    assert_eq!((white_standing.score, white_standing.color_balance), (2, 1));
    assert_eq!(
        (black_standing.score, black_standing.color_balance),
        (0, -1)
    );

    // Recording the same board again must be rejected (idempotency guard).
    let err = send(
        &mut ctx,
        record_swiss_result_ix(
            authority.pubkey(),
            0,
            0,
            white.pubkey(),
            black.pubkey(),
            SwissMatchResult::Draw,
        ),
        &[&authority],
    )
    .await?
    .err()
    .context("required operation value")?;
    assert_eq!(
        custom_code(&err),
        Some(ec(GameErrorCode::BoardAlreadyRecorded))
    );

    send(
        &mut ctx,
        advance_round_ix(authority.pubkey(), 1),
        &[&authority],
    )
    .await?
    .context("advance_round should succeed once the board is in")?;

    let t = fetch_tournament(&mut ctx).await?;
    assert_eq!(t.current_round, 1);
    assert_eq!(t.round_boards_reported, [0u8; 16]);

    // Nothing left to advance to (round == total_rounds).
    let err = send(
        &mut ctx,
        advance_round_ix(authority.pubkey(), 1),
        &[&authority],
    )
    .await?
    .err()
    .context("required operation value")?;
    assert_eq!(
        custom_code(&err),
        Some(ec(GameErrorCode::InvalidGameStatus))
    );
    Ok(())
}

#[tokio::test]
async fn players_and_third_parties_cannot_report_or_advance() -> anyhow::Result<()> {
    let authority = Keypair::new();
    let white = Keypair::new();
    let black = Keypair::new();
    let mut ctx = setup(authority.pubkey(), white.pubkey(), black.pubkey()).await?;

    // A player signing their own win is no longer accepted.
    let err = send(
        &mut ctx,
        record_swiss_result_ix(
            white.pubkey(),
            0,
            0,
            white.pubkey(),
            black.pubkey(),
            SwissMatchResult::WhiteWin,
        ),
        &[&white],
    )
    .await?
    .err()
    .context("required operation value")?;
    assert_eq!(
        custom_code(&err),
        Some(ec(GameErrorCode::NotTournamentAuthority))
    );

    let err = send(&mut ctx, advance_round_ix(black.pubkey(), 0), &[&black])
        .await?
        .err()
        .context("required operation value")?;
    assert_eq!(
        custom_code(&err),
        Some(ec(GameErrorCode::NotTournamentAuthority))
    );
    Ok(())
}

#[tokio::test]
async fn self_pairing_and_oversized_rounds_are_rejected() -> anyhow::Result<()> {
    let authority = Keypair::new();
    let white = Keypair::new();
    let black = Keypair::new();
    let mut ctx = setup(authority.pubkey(), white.pubkey(), black.pubkey()).await?;

    let err = send(
        &mut ctx,
        record_swiss_result_ix(
            authority.pubkey(),
            0,
            0,
            white.pubkey(),
            white.pubkey(),
            SwissMatchResult::WhiteWin,
        ),
        &[&authority],
    )
    .await?
    .err()
    .context("required operation value")?;
    assert_eq!(custom_code(&err), Some(ec(GameErrorCode::InvalidArgument)));

    // Two registered players means at most one board per round.
    let err = send(
        &mut ctx,
        advance_round_ix(authority.pubkey(), 2),
        &[&authority],
    )
    .await?
    .err()
    .context("required operation value")?;
    assert_eq!(custom_code(&err), Some(ec(GameErrorCode::InvalidArgument)));
    Ok(())
}

#[tokio::test]
async fn bye_is_a_full_point_for_one_player() -> anyhow::Result<()> {
    let authority = Keypair::new();
    let white = Keypair::new();
    let black = Keypair::new();
    let mut ctx = setup(authority.pubkey(), white.pubkey(), black.pubkey()).await?;

    // A bye names one player twice; two different players is a malformed bye.
    let err = send(
        &mut ctx,
        record_swiss_result_ix(
            authority.pubkey(),
            0,
            0,
            white.pubkey(),
            black.pubkey(),
            SwissMatchResult::Bye,
        ),
        &[&authority],
    )
    .await?
    .err()
    .context("required operation value")?;
    assert_eq!(custom_code(&err), Some(ec(GameErrorCode::InvalidArgument)));

    send(
        &mut ctx,
        record_swiss_result_ix(
            authority.pubkey(),
            0,
            0,
            black.pubkey(),
            black.pubkey(),
            SwissMatchResult::Bye,
        ),
        &[&authority],
    )
    .await?
    .context("bye should record")?;

    let shard = fetch_shard0(&mut ctx).await?;
    let bye = &shard.swiss_standings[1];
    assert_eq!((bye.score, bye.color_balance), (2, 0));
    assert_eq!(shard.swiss_standings[0].score, 0);
    Ok(())
}
