//! Who may fund, enter and spend inside a tournament.

use anyhow::Context as _;
pub mod common;

use anchor_lang::{AccountDeserialize, InstructionData, Space, ToAccountMetas};
use common::*;
use solana_sdk::{
    account::{Account, AccountSharedData},
    instruction::Instruction,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};
use xfchess_game::errors::GameErrorCode;
use xfchess_game::state::{
    get_default_prize_shares, Game, GameResult, GameStatus, GameType, MatchType, PayoutType,
    PlayerProfile, Tournament, TournamentPlayersShard, TournamentSessionDelegation,
    TournamentStatus, TournamentType,
};
use xfchess_game::tournament_ix::lifecycle::initialize_escrow::TournamentEscrow;

const TID: u64 = 77;
const GAME_ID: u64 = 7_700;

fn tournament_pda() -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"tournament", &TID.to_le_bytes()], &xfchess_game::ID)
}

fn t_escrow_pda() -> Pubkey {
    Pubkey::find_program_address(&[b"t_escrow", &TID.to_le_bytes()], &xfchess_game::ID).0
}

fn shard0_pda() -> Pubkey {
    Pubkey::find_program_address(
        &[b"tourney_players", &[0u8], &TID.to_le_bytes()],
        &xfchess_game::ID,
    )
    .0
}

fn tournament_session_pda(player: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"tournament_session", &TID.to_le_bytes(), player.as_ref()],
        &xfchess_game::ID,
    )
}

fn tournament(authority: Pubkey, host_treasury: Pubkey, status: TournamentStatus) -> Tournament {
    Tournament {
        tournament_id: TID,
        authority,
        name: String::new(),
        entry_fee: 0,
        platform_fee: 0,
        prize_pool: 0,
        max_players: 8,
        player_count: 0,
        num_registered_players: 0,
        status,
        start_time: None,
        end_time: None,
        fees_advanced: 0,
        fee_payer: authority,
        tournament_type: TournamentType::Swiss { rounds: 3 },
        current_round: 0,
        total_rounds: 3,
        total_matches: 0,
        final_match_index: 0,
        elo_min: 0,
        elo_max: u32::MAX,
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
        prize_shares: get_default_prize_shares(8, false),
        created_at: 0,
        started_at: None,
        completed_at: None,
        bump: tournament_pda().1,
        prizes_claimed: 0,
        platform_fee_pool: 0,
        usdc_prize_mint: None,
        usdc_prize_pool: 0,
        usdc_prize_funded: false,
        host_treasury,
        prize_token_mint: None,
        payout_type: PayoutType::LumpSum,
        vesting_params: None,
        // Blitz: registration reads the profile's blitz rating.
        base_time_seconds: 300,
        increment_seconds: 0,
        winner_takes_all: false,
        round_boards_reported: [0u8; 16],
    }
}

fn shard0(players: Vec<Pubkey>) -> anyhow::Result<Account> {
    Ok({
        let shard = TournamentPlayersShard {
            tournament_id: TID,
            shard_id: 0,
            player_elos: vec![1200; players.len()],
            players,
            swiss_standings: Vec::new(),
        };
        program_account(&shard, 8 + TournamentPlayersShard::space_for())?
    })
}

fn profile(authority: Pubkey, blitz: f64) -> anyhow::Result<Account> {
    Ok({
        let p = PlayerProfile {
            authority,
            elo_rating: 120_000.0,
            elo_blitz: blitz,
            ..Default::default()
        };
        program_account(&p, 8 + PlayerProfile::INIT_SPACE)?
    })
}

fn base_accounts(
    t: &Tournament,
    shard_players: Vec<Pubkey>,
) -> anyhow::Result<Vec<(Pubkey, Account)>> {
    Ok({
        vec![
            (
                tournament_pda().0,
                program_account(t, 8 + Tournament::INIT_SPACE)?,
            ),
            (t_escrow_pda(), program_account(&TournamentEscrow {}, 8)?),
            (shard0_pda(), shard0(shard_players)?),
            (t.host_treasury, system_account(1_000_000_000)),
        ]
    })
}

async fn fetch_tournament(
    ctx: &mut solana_program_test::ProgramTestContext,
) -> anyhow::Result<Tournament> {
    Ok({
        let acc = ctx
            .banks_client
            .get_account(tournament_pda().0)
            .await
            .context("required operation value")?
            .context("tournament account")?;
        Tournament::try_deserialize(&mut &acc.data[..]).context("required operation value")?
    })
}

fn fund_sol_prize_ix(operator: Pubkey, amount: u64) -> Instruction {
    let accounts = xfchess_game::__client_accounts_fund_sol_prize::FundSolPrize {
        tournament: tournament_pda().0,
        escrow_pda: t_escrow_pda(),
        operator,
        system_program: solana_system_interface::program::ID,
    }
    .to_account_metas(None);
    let data = xfchess_game::instruction::FundSolPrize {
        tournament_id: TID,
        amount,
    }
    .data();
    Instruction {
        program_id: xfchess_game::ID,
        accounts,
        data,
    }
}

fn register_ix(player: Pubkey, host_treasury: Pubkey, claimed_elo: u32) -> Instruction {
    let accounts = xfchess_game::__client_accounts_register_player::RegisterPlayer {
        tournament: tournament_pda().0,
        player_profile: profile_pda(&player).0,
        player,
        escrow_pda: t_escrow_pda(),
        tournament_players_shard_0: shard0_pda(),
        tournament_players_shard_1: None,
        tournament_players_shard_2: None,
        tournament_players_shard_3: None,
        host_treasury,
        system_program: solana_system_interface::program::ID,
    }
    .to_account_metas(None);
    let data = xfchess_game::instruction::RegisterPlayer {
        tournament_id: TID,
        elo: claimed_elo,
    }
    .data();
    Instruction {
        program_id: xfchess_game::ID,
        accounts,
        data,
    }
}

#[tokio::test]
async fn only_the_tournament_authority_funds_the_guaranteed_prize() -> anyhow::Result<()> {
    let authority = Keypair::new();
    let t = tournament(
        authority.pubkey(),
        Pubkey::new_unique(),
        TournamentStatus::Registration,
    );
    let mut accounts = base_accounts(&t, Vec::new())?;
    accounts.push((authority.pubkey(), system_account(5_000_000_000)));
    let mut ctx = start(accounts).await;

    // A front-runner locking in a token prize would block the real one.
    let outsider = ctx.payer.pubkey();
    let err = send(&mut ctx, fund_sol_prize_ix(outsider, 1), &[])
        .await
        .err()
        .context("an outsider must not fund the prize")?;
    assert_eq!(
        custom_code(&err),
        Some(ec(GameErrorCode::NotTournamentAuthority))
    );
    assert_eq!(fetch_tournament(&mut ctx).await?.prize_pool, 0);

    send(
        &mut ctx,
        fund_sol_prize_ix(authority.pubkey(), 1_000_000_000),
        &[&authority],
    )
    .await
    .context("the authority funds the prize")?;
    assert_eq!(fetch_tournament(&mut ctx).await?.prize_pool, 1_000_000_000);
    Ok(())
}

#[tokio::test]
async fn registration_rating_comes_from_the_profile_not_the_caller() -> anyhow::Result<()> {
    let host = Pubkey::new_unique();
    let mut t = tournament(Pubkey::new_unique(), host, TournamentStatus::Registration);
    t.elo_min = 1500;
    t.elo_max = 2000;
    let mut ctx = start(base_accounts(&t, Vec::new())?).await;
    let player = ctx.payer.pubkey();

    // A 1200-rated player cannot claim 1600 to enter a 1500-2000 event.
    ctx.set_account(
        &profile_pda(&player).0,
        &AccountSharedData::from(profile(player, 120_000.0)?),
    );
    let err = send(&mut ctx, register_ix(player, host, 1600), &[])
        .await
        .err()
        .context("a claimed rating must not bypass the ELO range")?;
    assert_eq!(custom_code(&err), Some(ec(GameErrorCode::EloOutOfRange)));

    // A genuinely 1600-rated player enters whatever the client sends, and is
    // seeded by the real rating.
    ctx.set_account(
        &profile_pda(&player).0,
        &AccountSharedData::from(profile(player, 160_000.0)?),
    );
    send(&mut ctx, register_ix(player, host, 0), &[])
        .await
        .context("an eligible profile registers")?;
    let shard = ctx
        .banks_client
        .get_account(shard0_pda())
        .await
        .context("required operation value")?
        .context("required operation value")?;
    let shard = TournamentPlayersShard::try_deserialize(&mut &shard.data[..])
        .context("required operation value")?;
    assert_eq!(shard.players, vec![player]);
    assert_eq!(shard.player_elos, vec![1600]);
    Ok(())
}

fn waiting_game(white: Pubkey, tournament_id: Option<u64>) -> anyhow::Result<Account> {
    Ok({
        let game = Game {
            game_id: GAME_ID,
            white,
            black: Pubkey::default(),
            status: GameStatus::WaitingForOpponent,
            last_move_timestamp: 0,
            fees_advanced: 0,
            fee_payer: white,
            result: GameResult::None,
            board_state: start_board(),
            move_count: 0,
            halfmove_clock: 0,
            turn: 1,
            created_at: 0,
            updated_at: 0,
            wager_amount: 10_000_000,
            wager_token: None,
            game_type: GameType::PvP,
            match_type: MatchType::Rated,
            country_fee: 0,
            base_time_seconds: 300,
            increment_seconds: 0,
            bump: game_pda(GAME_ID).1,
            is_delegated: false,
            tournament_id,
            nonce: 0,
            draw_offered_by: None,
        };
        program_account(&game, 8 + Game::INIT_SPACE)?
    })
}

fn session_join_ix(white: Pubkey, player: Pubkey, session_signer: Pubkey) -> Instruction {
    let accounts = xfchess_game::__client_accounts_session_join_game::SessionJoinGame {
        tournament: tournament_pda().0,
        tournament_players_shard_0: shard0_pda(),
        tournament_players_shard_1: None,
        tournament_players_shard_2: None,
        tournament_players_shard_3: None,
        session_delegation: tournament_session_pda(&player).0,
        session_signer,
        player,
        game: game_pda(GAME_ID).0,
        escrow_pda: Pubkey::find_program_address(
            &[b"escrow", &GAME_ID.to_le_bytes()],
            &xfchess_game::ID,
        )
        .0,
        white_profile: profile_pda(&white).0,
        player_profile: profile_pda(&player).0,
        system_program: solana_system_interface::program::ID,
    }
    .to_account_metas(None);
    let data = xfchess_game::instruction::SessionJoinGame {
        tournament_id: TID,
        game_id: GAME_ID,
    }
    .data();
    Instruction {
        program_id: xfchess_game::ID,
        accounts,
        data,
    }
}

async fn session_join(
    game_tournament: Option<u64>,
) -> anyhow::Result<(Result<(), solana_sdk::transaction::TransactionError>, Game)> {
    Ok({
        let creator = Pubkey::new_unique();
        let joiner = Pubkey::new_unique();
        let session = Keypair::new();
        let t = tournament(
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            TournamentStatus::Active,
        );

        let (session_pda, session_bump) = tournament_session_pda(&joiner);
        let delegation = TournamentSessionDelegation {
            tournament_id: TID,
            player: joiner,
            session_key: session.pubkey(),
            expires_at: i64::MAX,
            spending_limit: u64::MAX,
            total_spent: 0,
            max_wager: u64::MAX,
            games_played: 0,
            enabled: true,
            bump: session_bump,
        };
        let mut accounts = base_accounts(&t, vec![creator, joiner])?;
        accounts.extend([
            (
                session_pda,
                program_account(&delegation, 8 + TournamentSessionDelegation::INIT_SPACE)?,
            ),
            (game_pda(GAME_ID).0, waiting_game(creator, game_tournament)?),
            (profile_pda(&creator).0, profile(creator, 0.0)?),
            (profile_pda(&joiner).0, profile(joiner, 0.0)?),
        ]);
        let mut ctx = start(accounts).await;

        let ix = session_join_ix(creator, joiner, session.pubkey());
        let result = send(&mut ctx, ix, &[&session]).await;
        (result, fetch_game(&mut ctx, GAME_ID).await?)
    })
}

#[tokio::test]
async fn tournament_session_cannot_join_a_game_outside_its_tournament() -> anyhow::Result<()> {
    let (result, game) = session_join(None).await?;
    let err = result
        .err()
        .context("the vault was funded for this tournament only")?;
    assert_eq!(custom_code(&err), Some(ec(GameErrorCode::InvalidSession)));
    assert_eq!(game.status, GameStatus::WaitingForOpponent);
    Ok(())
}

#[tokio::test]
async fn tournament_session_joins_its_own_tournament_game() -> anyhow::Result<()> {
    let (result, game) = session_join(Some(TID)).await?;
    result.context("a tournament game may be joined with the tournament session")?;
    assert_eq!(game.status, GameStatus::Active);
    Ok(())
}
