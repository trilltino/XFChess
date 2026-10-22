use anchor_lang::{AccountSerialize, InstructionData, Space, ToAccountMetas};
use anyhow::Context as _;
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
    get_default_prize_shares, Game, GameStatus, PayoutType, SessionDelegation, SwissStanding,
    Tournament, TournamentPlayersShard, TournamentStatus, TournamentType,
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

const GAME_ID: u64 = (TOURNAMENT_ID << 16) | (1 << 8) | 1;

fn game_pda(game_id: u64) -> Pubkey {
    Pubkey::find_program_address(&[b"game", &game_id.to_le_bytes()], &xfchess_game::ID).0
}

fn session_pda(game_id: u64, player: Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[
            b"session_delegation",
            &game_id.to_le_bytes(),
            player.as_ref(),
        ],
        &xfchess_game::ID,
    )
    .0
}

fn create_tournament_game_ix(
    authority: Pubkey,
    white: Pubkey,
    black: Pubkey,
    session_key: Pubkey,
) -> Instruction {
    let accounts = xfchess_game::__client_accounts_create_tournament_game::CreateTournamentGame {
        tournament: tournament_pda(TOURNAMENT_ID).0,
        tournament_players_shard_0: shard_pda(TOURNAMENT_ID, 0).0,
        tournament_players_shard_1: None,
        tournament_players_shard_2: None,
        tournament_players_shard_3: None,
        game: game_pda(GAME_ID),
        white_session: session_pda(GAME_ID, white),
        black_session: session_pda(GAME_ID, black),
        white,
        black,
        authority,
        system_program: solana_system_interface::program::ID,
    }
    .to_account_metas(None);
    let data = xfchess_game::instruction::CreateTournamentGame {
        tournament_id: TOURNAMENT_ID,
        game_id: GAME_ID,
        session_key,
    }
    .data();
    Instruction {
        program_id: xfchess_game::ID,
        accounts,
        data,
    }
}

struct Field {
    authority: Keypair,
    p1: Pubkey,
    p2: Pubkey,
    p3: Pubkey,
    p4: Pubkey,
}

async fn setup() -> anyhow::Result<(ProgramTestContext, Field)> {
    Ok({
        let field = Field {
            authority: Keypair::new(),
            p1: Pubkey::new_unique(),
            p2: Pubkey::new_unique(),
            p3: Pubkey::new_unique(),
            p4: Pubkey::new_unique(),
        };
        let (_, t_bump) = tournament_pda(TOURNAMENT_ID);
        let mut ctx = start(vec![
            (
                tournament_pda(TOURNAMENT_ID).0,
                serialize_padded(
                    &tournament(field.authority.pubkey(), field.p1, t_bump, 0),
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
        // The authority pays rent for the game and both session delegations.
        let fund = solana_system_interface::instruction::transfer(
            &ctx.payer.pubkey(),
            &field.authority.pubkey(),
            1_000_000_000,
        );
        send(&mut ctx, fund, &[])
            .await?
            .context("required operation value")?;
        (ctx, field)
    })
}

async fn fetch<T: anchor_lang::AccountDeserialize>(
    ctx: &mut ProgramTestContext,
    key: Pubkey,
) -> anyhow::Result<T> {
    Ok({
        let acc = ctx
            .banks_client
            .get_account(key)
            .await
            .context("required operation value")?
            .context("account missing")?;
        T::try_deserialize(&mut &acc.data[..]).context("required operation value")?
    })
}

#[tokio::test]
async fn authority_seats_a_pairing_in_an_active_game_with_move_sessions() -> anyhow::Result<()> {
    let (mut ctx, f) = setup().await?;
    let session_key = Pubkey::new_unique();

    send(
        &mut ctx,
        create_tournament_game_ix(f.authority.pubkey(), f.p1, f.p3, session_key),
        &[&f.authority],
    )
    .await?
    .context("create_tournament_game should succeed")?;

    let game: Game = fetch(&mut ctx, game_pda(GAME_ID)).await?;
    assert_eq!(game.game_id, GAME_ID);
    assert_eq!((game.white, game.black), (f.p1, f.p3));
    assert_eq!(game.status, GameStatus::Active);
    assert_eq!(game.tournament_id, Some(TOURNAMENT_ID));
    assert_eq!(game.wager_amount, 0);
    assert_eq!(game.fee_payer, session_key);

    for player in [f.p1, f.p3] {
        let s: SessionDelegation = fetch(&mut ctx, session_pda(GAME_ID, player)).await?;
        assert_eq!(
            (s.game_id, s.player, s.session_key),
            (GAME_ID, player, session_key)
        );
        assert!(s.enabled);
    }

    // The same game id cannot be created twice.
    let err = send(
        &mut ctx,
        create_tournament_game_ix(f.authority.pubkey(), f.p2, f.p4, session_key),
        &[&f.authority],
    )
    .await?;
    assert!(err.is_err());
    Ok(())
}

#[tokio::test]
async fn only_the_authority_may_create_and_only_for_registered_players() -> anyhow::Result<()> {
    let (mut ctx, f) = setup().await?;
    let session_key = Pubkey::new_unique();

    let outsider = Keypair::new();
    let fund = solana_system_interface::instruction::transfer(
        &ctx.payer.pubkey(),
        &outsider.pubkey(),
        1_000_000_000,
    );
    send(&mut ctx, fund, &[])
        .await?
        .context("required operation value")?;
    let err = send(
        &mut ctx,
        create_tournament_game_ix(outsider.pubkey(), f.p1, f.p2, session_key),
        &[&outsider],
    )
    .await?
    .err()
    .context("required operation value")?;
    assert_eq!(
        custom_code(&err),
        Some(ec(GameErrorCode::NotTournamentAuthority))
    );

    let err = send(
        &mut ctx,
        create_tournament_game_ix(
            f.authority.pubkey(),
            f.p1,
            Pubkey::new_unique(),
            session_key,
        ),
        &[&f.authority],
    )
    .await?
    .err()
    .context("required operation value")?;
    assert_eq!(custom_code(&err), Some(ec(GameErrorCode::PlayerNotFound)));
    // Self-pairing is rejected (the two seats would share one session PDA).
    send(
        &mut ctx,
        create_tournament_game_ix(f.authority.pubkey(), f.p1, f.p1, session_key),
        &[&f.authority],
    )
    .await?
    .err()
    .context("required operation value")?;
    Ok(())
}
