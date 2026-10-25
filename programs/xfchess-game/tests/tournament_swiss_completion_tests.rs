#[path = "common/agave.rs"]
pub mod agave;
use crate::agave::{BanksClientError, ProgramTest, ProgramTestBanksClientExt, ProgramTestContext};
use anchor_lang::{AccountSerialize, InstructionData, Space, ToAccountMetas};
use anyhow::Context as _;
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

fn tournament(authority: Pubkey, fee_payer: Pubkey, bump: u8, current_round: u8) -> Tournament {
    Tournament {
        tournament_id: TOURNAMENT_ID,
        authority,
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

fn complete_swiss_tournament_ix(authority: Pubkey, places: Vec<Pubkey>) -> Instruction {
    let accounts =
        xfchess_game::__client_accounts_complete_swiss_tournament::CompleteSwissTournament {
            tournament: tournament_pda(TOURNAMENT_ID).0,
            tournament_players_shard_0: shard_pda(TOURNAMENT_ID, 0).0,
            tournament_players_shard_1: None,
            tournament_players_shard_2: None,
            tournament_players_shard_3: None,
            authority,
        }
        .to_account_metas(None);
    let data = xfchess_game::instruction::CompleteSwissTournament {
        tournament_id: TOURNAMENT_ID,
        places,
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

struct Field {
    authority: Keypair,
    p1: Pubkey,
    p2: Pubkey,
    p3: Pubkey,
    p4: Pubkey,
}

async fn setup(current_round: u8) -> anyhow::Result<(ProgramTestContext, Field)> {
    Ok({
        let field = Field {
            authority: Keypair::new(),
            p1: Pubkey::new_unique(),
            p2: Pubkey::new_unique(),
            p3: Pubkey::new_unique(),
            p4: Pubkey::new_unique(),
        };
        let (_, t_bump) = tournament_pda(TOURNAMENT_ID);
        let ctx = start(vec![
            (
                tournament_pda(TOURNAMENT_ID).0,
                serialize_padded(
                    &tournament(field.authority.pubkey(), field.p1, t_bump, current_round),
                    8 + Tournament::INIT_SPACE,
                )?,
            ),
            (
                shard_pda(TOURNAMENT_ID, 0).0,
                serialize_padded(
                    &shard0(field.p1, field.p2, field.p3, field.p4),
                    8 + TournamentPlayersShard::space_for(),
                )?,
            ),
        ])
        .await;
        (ctx, field)
    })
}

#[tokio::test]
async fn completes_with_authority_tiebreak_order() -> anyhow::Result<()> {
    // current_round == total_rounds: both rounds already advanced.
    let (mut ctx, f) = setup(2).await?;

    // p2 and p3 tie on score (2); the off-chain tiebreak puts p3 first, and the
    // program accepts any order that respects on-chain scores.
    send(
        &mut ctx,
        complete_swiss_tournament_ix(f.authority.pubkey(), vec![f.p1, f.p3, f.p2, f.p4]),
        &[&f.authority],
    )
    .await?
    .context("complete_swiss_tournament should succeed once all rounds are in")?;

    let t = fetch_tournament(&mut ctx).await?;
    assert_eq!(t.status, TournamentStatus::Completed);
    assert!(t.completed_at.is_some());
    assert_eq!(t.winner, Some(f.p1));
    assert_eq!(t.second_place, Some(f.p3));
    assert_eq!(t.third_place, Some(f.p2));
    assert_eq!(t.fourth_place, Some(f.p4));
    assert_eq!(t.fifth_place, None);
    Ok(())
}

#[tokio::test]
async fn rejects_completion_before_final_round_advances() -> anyhow::Result<()> {
    // Only round 0 of 2 has been advanced past.
    let (mut ctx, f) = setup(1).await?;

    let err = send(
        &mut ctx,
        complete_swiss_tournament_ix(f.authority.pubkey(), vec![f.p1, f.p2, f.p3, f.p4]),
        &[&f.authority],
    )
    .await?
    .err()
    .context("required operation value")?;
    assert_eq!(
        custom_code(&err),
        Some(ec(GameErrorCode::SwissTournamentNotFinished))
    );
    Ok(())
}

#[tokio::test]
async fn rejects_places_that_contradict_scores() -> anyhow::Result<()> {
    let (mut ctx, f) = setup(2).await?;

    let bad_orders = [
        vec![f.p2, f.p1, f.p3, f.p4], // lower score above a higher one
        vec![f.p1, f.p1, f.p2, f.p3], // duplicate
        vec![f.p1, f.p2, f.p3],       // wrong length
        vec![f.p1, f.p2, f.p3, Pubkey::new_unique()], // not registered
    ];
    for places in bad_orders {
        let err = send(
            &mut ctx,
            complete_swiss_tournament_ix(f.authority.pubkey(), places.clone()),
            &[&f.authority],
        )
        .await?
        .err()
        .context("required operation value")?;
        assert_eq!(
            custom_code(&err),
            Some(ec(GameErrorCode::InvalidSwissPlaces)),
            "places {places:?}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn rejects_completion_by_non_authority() -> anyhow::Result<()> {
    let (mut ctx, f) = setup(2).await?;
    let outsider = Keypair::new();

    let err = send(
        &mut ctx,
        complete_swiss_tournament_ix(outsider.pubkey(), vec![f.p1, f.p2, f.p3, f.p4]),
        &[&outsider],
    )
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
async fn rejects_completion_that_omits_a_required_shard() -> anyhow::Result<()> {
    let authority = Keypair::new();
    let (p1, p2, p3, p4) = (
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        Pubkey::new_unique(),
    );
    let (_, t_bump) = tournament_pda(TOURNAMENT_ID);
    // 128 seats need shards 0 and 1; players in the missing shard 1 could
    // outscore anyone in the submitted order.
    let mut t = tournament(authority.pubkey(), p1, t_bump, 2);
    t.max_players = 128;
    let mut ctx = start(vec![
        (
            tournament_pda(TOURNAMENT_ID).0,
            serialize_padded(&t, 8 + Tournament::INIT_SPACE)?,
        ),
        (
            shard_pda(TOURNAMENT_ID, 0).0,
            serialize_padded(
                &shard0(p1, p2, p3, p4),
                8 + TournamentPlayersShard::space_for(),
            )?,
        ),
    ])
    .await;

    let err = send(
        &mut ctx,
        complete_swiss_tournament_ix(authority.pubkey(), vec![p1, p3, p2, p4]),
        &[&authority],
    )
    .await?
    .err()
    .context("completion must see every shard the field occupies")?;
    assert_eq!(
        custom_code(&err),
        Some(ec(GameErrorCode::InvalidSwissPlaces))
    );
    assert_eq!(
        fetch_tournament(&mut ctx).await?.status,
        TournamentStatus::Active
    );
    Ok(())
}
