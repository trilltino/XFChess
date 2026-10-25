#[path = "common/agave.rs"]
pub mod agave;
use anyhow::Context as _;
pub mod common;

use anchor_lang::{AccountDeserialize, InstructionData, Space, ToAccountMetas};
use common::*;
use solana_sdk::{instruction::Instruction, pubkey::Pubkey, signature::Keypair, signer::Signer};
use xfchess_game::state::{
    Game, GameResult, GameStatus, GameType, GlobalSessionDelegation, MatchType, PlayerProfile,
};

fn global_session_pda(player: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[GlobalSessionDelegation::SEED, player.as_ref()],
        &xfchess_game::ID,
    )
}

fn escrow_pda(game_id: u64) -> Pubkey {
    Pubkey::find_program_address(&[b"escrow", &game_id.to_le_bytes()], &xfchess_game::ID).0
}

fn global_session_account(
    player: Pubkey,
    session_key: Pubkey,
    spending_limit: u64,
    max_wager: u64,
) -> anyhow::Result<(Pubkey, solana_sdk::account::Account)> {
    Ok({
        let (pda, bump) = global_session_pda(&player);
        let session = GlobalSessionDelegation {
            player,
            session_key,
            expires_at: i64::MAX / 2,
            spending_limit,
            total_spent: 0,
            max_wager,
            games_remaining: 200,
            enabled: true,
            bump,
        };
        (
            pda,
            program_account(&session, 8 + GlobalSessionDelegation::INIT_SPACE)?,
        )
    })
}

#[allow(clippy::too_many_arguments)]
fn global_create_game_ix(
    session_pda: Pubkey,
    session_signer: Pubkey,
    player: Pubkey,
    game_id: u64,
    wager_amount: u64,
    match_type: MatchType,
    platform_fee: u64,
    base_time_seconds: u64,
    increment_seconds: u16,
) -> Instruction {
    let mut accounts = xfchess_game::__client_accounts_global_create_game::GlobalCreateGame {
        session_delegation: session_pda,
        session_signer,
        player,
        game: game_pda(game_id).0,
        escrow_pda: escrow_pda(game_id),
        system_program: solana_system_interface::program::ID,
    }
    .to_account_metas(None);
    accounts.push(solana_sdk::instruction::AccountMeta::new(
        Pubkey::find_program_address(
            &[
                xfchess_game::state::RecoveryIdentity::SEED,
                &game_id.to_le_bytes(),
            ],
            &xfchess_game::ID,
        )
        .0,
        false,
    ));
    let data = xfchess_game::instruction::GlobalCreateGameV2 {
        game_id,
        wager_amount,
        match_type,
        platform_fee,
        base_time_seconds,
        increment_seconds,
    }
    .data();
    Instruction {
        program_id: xfchess_game::ID,
        accounts,
        data,
    }
}

#[tokio::test]
async fn global_create_game_funds_rent_and_wager_from_session_vault() -> anyhow::Result<()> {
    let player = Pubkey::new_unique();
    let session_signer = Keypair::new();
    let wager_amount = 10_000_000u64; // 0.01 SOL â€” above MIN_WAGER_LAMPORTS

    let (session_pda, session_account_data) = global_session_account(
        player,
        session_signer.pubkey(),
        5_000_000_000,
        1_000_000_000,
    )?;

    let mut ctx = start(vec![(session_pda, session_account_data)]).await;

    let game_id = 42u64;
    let ix = global_create_game_ix(
        session_pda,
        session_signer.pubkey(),
        player,
        game_id,
        wager_amount,
        MatchType::Rated,
        0,
        300,
        0,
    );

    send(&mut ctx, ix, &[&session_signer]).await.context(
        "global_create_game should succeed when the session vault PDA funds rent + wager",
    )?;

    let game = fetch_game(&mut ctx, game_id).await?;
    assert_eq!(game.game_id, game_id);
    assert_eq!(game.white, player);
    assert_eq!(game.black, Pubkey::default());
    assert_eq!(game.status, GameStatus::WaitingForOpponent);
    assert_eq!(game.wager_amount, wager_amount);
    assert_eq!(game.fee_payer, session_signer.pubkey());

    let escrow_balance = ctx
        .banks_client
        .get_balance(escrow_pda(game_id))
        .await
        .context("required operation value")?;
    assert_eq!(escrow_balance, wager_amount);

    let session_acc = ctx
        .banks_client
        .get_account(session_pda)
        .await
        .context("required operation value")?
        .context("session delegation account missing")?;
    let session = GlobalSessionDelegation::try_deserialize(&mut &session_acc.data[..])
        .context("required operation value")?;
    assert_eq!(session.total_spent, wager_amount);
    assert_eq!(session.games_remaining, 199);
    use xfchess_game::state::{RecoveryIdentity, RecoveryStatus};
    let identity_key = Pubkey::find_program_address(
        &[RecoveryIdentity::SEED, &game_id.to_le_bytes()],
        &xfchess_game::ID,
    )
    .0;
    let identity = ctx
        .banks_client
        .get_account(identity_key)
        .await?
        .context("v2 recovery identity missing")?;
    let identity = RecoveryIdentity::try_deserialize(&mut identity.data.as_slice())?;
    assert_eq!(identity.white, player);
    assert_eq!(identity.black, Pubkey::default());
    assert_eq!(identity.wager_amount, wager_amount);
    assert_eq!(identity.status, RecoveryStatus::Waiting);
    Ok(())
}

#[tokio::test]
async fn global_create_game_rejects_unreasonable_platform_fee() -> anyhow::Result<()> {
    let player = Pubkey::new_unique();
    let session_signer = Keypair::new();
    let wager_amount = 10_000_000u64;

    let (session_pda, session_account_data) = global_session_account(
        player,
        session_signer.pubkey(),
        5_000_000_000,
        1_000_000_000,
    )?;

    let mut ctx = start(vec![(session_pda, session_account_data)]).await;

    let game_id = 43u64;
    let absurd_platform_fee = 2_000_000_000u64; // 2 SOL â€” far above any real per-game fee
    let ix = global_create_game_ix(
        session_pda,
        session_signer.pubkey(),
        player,
        game_id,
        wager_amount,
        MatchType::Rated,
        absurd_platform_fee,
        300,
        0,
    );

    let err = send(&mut ctx, ix, &[&session_signer])
        .await
        .err()
        .context("an absurdly large platform_fee must be rejected")?;

    assert_eq!(
        custom_code(&err),
        Some(ec(xfchess_game::errors::GameErrorCode::PlatformFeeTooLarge)),
        "expected PlatformFeeTooLarge, got {err:?}"
    );
    Ok(())
}

#[tokio::test]
async fn global_create_game_rejects_an_underfunded_session_vault() -> anyhow::Result<()> {
    let player = Pubkey::new_unique();
    let session_signer = Keypair::new();
    let wager_amount = 10_000_000u64;

    // Caps are deliberately generous, so `has_budget` passes and the ONLY thing
    // that can reject this is the balance check.
    let (session_pda, mut session_account_data) = global_session_account(
        player,
        session_signer.pubkey(),
        5_000_000_000,
        1_000_000_000,
    )?;

    // Enough to keep the delegation account itself rent-exempt, but nowhere
    // near enough to also fund the new Game PDA's rent plus the wager.
    session_account_data.lamports = 2_000_000;

    let mut ctx = start(vec![(session_pda, session_account_data)]).await;

    let game_id = 45u64;
    let ix = global_create_game_ix(
        session_pda,
        session_signer.pubkey(),
        player,
        game_id,
        wager_amount,
        MatchType::Rated,
        0,
        300,
        0,
    );

    let err = send(&mut ctx, ix, &[&session_signer])
        .await
        .err()
        .context("an underfunded session vault must be rejected up front")?;

    assert_eq!(
        custom_code(&err),
        Some(ec(
            xfchess_game::errors::GameErrorCode::GlobalSessionVaultUnderfunded
        )),
        "expected GlobalSessionVaultUnderfunded, got {err:?}"
    );
    Ok(())
}

#[tokio::test]
async fn global_create_game_accepts_a_vault_funded_exactly_enough() -> anyhow::Result<()> {
    let player = Pubkey::new_unique();
    let session_signer = Keypair::new();
    let wager_amount = 10_000_000u64;

    let (session_pda, mut session_account_data) = global_session_account(
        player,
        session_signer.pubkey(),
        5_000_000_000,
        1_000_000_000,
    )?;

    // Both permanent accounts must be funded while preserving the vault reserve.
    let rent = anchor_lang::prelude::Rent::default();
    let reserve = rent.minimum_balance(session_account_data.data.len());
    session_account_data.lamports = reserve
        + rent.minimum_balance(8 + Game::INIT_SPACE)
        + rent.minimum_balance(8 + xfchess_game::state::RecoveryIdentity::INIT_SPACE)
        + wager_amount;

    let mut ctx = start(vec![(session_pda, session_account_data)]).await;

    let game_id = 46u64;
    let ix = global_create_game_ix(
        session_pda,
        session_signer.pubkey(),
        player,
        game_id,
        wager_amount,
        MatchType::Rated,
        0,
        300,
        0,
    );

    send(&mut ctx, ix, &[&session_signer])
        .await
        .context("a vault holding rent + wager + rent-exemption must be accepted")?;

    let escrow_balance = ctx
        .banks_client
        .get_balance(escrow_pda(game_id))
        .await
        .context("required operation value")?;
    assert_eq!(escrow_balance, wager_amount);
    assert_eq!(ctx.banks_client.get_balance(session_pda).await?, reserve);
    Ok(())
}

#[tokio::test]
async fn global_create_game_works_for_a_free_zero_wager_game() -> anyhow::Result<()> {
    // The PDA vault must fund account rent even for a zero-wager game.
    let player = Pubkey::new_unique();
    let session_signer = Keypair::new();

    let (session_pda, session_account_data) = global_session_account(
        player,
        session_signer.pubkey(),
        5_000_000_000,
        1_000_000_000,
    )?;

    let mut ctx = start(vec![(session_pda, session_account_data)]).await;

    let game_id = 7u64;
    let ix = global_create_game_ix(
        session_pda,
        session_signer.pubkey(),
        player,
        game_id,
        0,
        MatchType::Free,
        0,
        300,
        0,
    );

    send(&mut ctx, ix, &[&session_signer])
        .await
        .context("global_create_game should succeed for a zero-wager game")?;

    let game = fetch_game(&mut ctx, game_id).await?;
    assert_eq!(game.wager_amount, 0);
    assert_eq!(game.status, GameStatus::WaitingForOpponent);
    Ok(())
}

fn player_profile_pda(player: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"profile", player.as_ref()], &xfchess_game::ID).0
}

#[tokio::test]
async fn wallet_v2_create_join_cancel_conserves_wagers_and_closes_identity() -> anyhow::Result<()> {
    use crate::agave::ProgramTestBanksClientExt;
    use xfchess_game::state::{RecoveryIdentity, RecoveryStatus};
    let white = Keypair::new();
    let black = Keypair::new();
    let mut ctx = start(vec![
        (white.pubkey(), system_account(1_000_000_000)),
        (black.pubkey(), system_account(1_000_000_000)),
        player_profile_account(white.pubkey())?,
        player_profile_account(black.pubkey())?,
    ])
    .await;
    let game_id = 501u64;
    let wager = 10_000_000;
    let record_key = Pubkey::find_program_address(
        &[RecoveryIdentity::SEED, &game_id.to_le_bytes()],
        &xfchess_game::ID,
    )
    .0;
    let payer = ctx.payer.pubkey();
    let mut create_accounts = xfchess_game::__client_accounts_create_game::CreateGame {
        game: game_pda(game_id).0,
        escrow_pda: escrow_pda(game_id),
        player: white.pubkey(),
        fee_payer: payer,
        system_program: solana_system_interface::program::ID,
    }
    .to_account_metas(None);
    create_accounts.push(solana_sdk::instruction::AccountMeta::new(record_key, false));
    send(
        &mut ctx,
        Instruction {
            program_id: xfchess_game::ID,
            accounts: create_accounts,
            data: xfchess_game::instruction::CreateGameV2 {
                game_id,
                wager_amount: wager,
                match_type: MatchType::Rated,
                platform_fee: 0,
                base_time_seconds: 300,
                increment_seconds: 0,
            }
            .data(),
        },
        &[&white],
    )
    .await?;
    assert_eq!(
        ctx.banks_client.get_balance(white.pubkey()).await?,
        1_000_000_000 - wager
    );
    let mut join_accounts = xfchess_game::__client_accounts_join_game::JoinGame {
        game: game_pda(game_id).0,
        escrow_pda: escrow_pda(game_id),
        player_profile: player_profile_pda(&black.pubkey()),
        white_profile: player_profile_pda(&white.pubkey()),
        player: black.pubkey(),
        fee_payer: payer,
        system_program: solana_system_interface::program::ID,
    }
    .to_account_metas(None);
    join_accounts.push(solana_sdk::instruction::AccountMeta::new(record_key, false));
    send(
        &mut ctx,
        Instruction {
            program_id: xfchess_game::ID,
            accounts: join_accounts,
            data: xfchess_game::instruction::JoinGameV2 { game_id }.data(),
        },
        &[&black],
    )
    .await?;
    assert_eq!(
        ctx.banks_client.get_balance(black.pubkey()).await?,
        1_000_000_000 - wager
    );
    assert_eq!(
        ctx.banks_client.get_balance(escrow_pda(game_id)).await?,
        2 * wager
    );
    let before = ctx
        .banks_client
        .get_account(record_key)
        .await?
        .context("record missing")?;
    let ready = RecoveryIdentity::try_deserialize(&mut before.data.as_slice())?;
    assert_eq!(
        (ready.white, ready.black, ready.status),
        (white.pubkey(), black.pubkey(), RecoveryStatus::Ready)
    );
    let mut cancel_accounts = xfchess_game::__client_accounts_cancel_game::CancelGame {
        game: game_pda(game_id).0,
        escrow_pda: escrow_pda(game_id),
        player: white.pubkey(),
        white_authority: white.pubkey(),
        black_authority: black.pubkey(),
        system_program: solana_system_interface::program::ID,
    }
    .to_account_metas(None);
    // The legacy optional recipient is writable only after an opponent joins.
    cancel_accounts[4].is_writable = true;
    let cancel = Instruction {
        program_id: xfchess_game::ID,
        accounts: cancel_accounts,
        data: xfchess_game::instruction::CancelGame { game_id }.data(),
    };
    send(&mut ctx, cancel.clone(), &[&white]).await?;
    // A fresh message executes the handler again instead of reusing a cached tx.
    ctx.last_blockhash = ctx
        .banks_client
        .get_new_latest_blockhash(&ctx.last_blockhash)
        .await?;
    send(&mut ctx, cancel, &[&white]).await?;
    for player in [white.pubkey(), black.pubkey()] {
        assert_eq!(ctx.banks_client.get_balance(player).await?, 1_000_000_000);
    }
    assert_eq!(ctx.banks_client.get_balance(escrow_pda(game_id)).await?, 0);
    let after = ctx
        .banks_client
        .get_account(record_key)
        .await?
        .context("permanent record missing")?;
    let closed = RecoveryIdentity::try_deserialize(&mut after.data.as_slice())?;
    assert_eq!(closed.status, RecoveryStatus::Closed);
    assert_eq!(after.lamports, before.lamports);
    Ok(())
}

fn player_profile_account(
    authority: Pubkey,
) -> anyhow::Result<(Pubkey, solana_sdk::account::Account)> {
    Ok({
        let pda = player_profile_pda(&authority);
        let profile = PlayerProfile {
            authority,
            ..Default::default()
        };
        (
            pda,
            program_account(&profile, 8 + PlayerProfile::INIT_SPACE)?,
        )
    })
}

fn waiting_game_account(
    game_id: u64,
    white: Pubkey,
    wager_amount: u64,
) -> anyhow::Result<(Pubkey, solana_sdk::account::Account)> {
    Ok({
        let (pda, bump) = game_pda(game_id);
        let game = Game {
            game_id,
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
            wager_amount,
            wager_token: None,
            game_type: GameType::PvP,
            match_type: MatchType::Rated,
            country_fee: 0,
            base_time_seconds: 300,
            increment_seconds: 0,
            bump,
            is_delegated: false,
            tournament_id: None,
            nonce: 0,
            draw_offered_by: None,
        };
        (pda, program_account(&game, 8 + Game::INIT_SPACE)?)
    })
}

fn global_join_game_ix(
    session_pda: Pubkey,
    session_signer: Pubkey,
    player: Pubkey,
    white: Pubkey,
    game_id: u64,
) -> Instruction {
    let accounts = xfchess_game::__client_accounts_global_join_game::GlobalJoinGame {
        session_delegation: session_pda,
        session_signer,
        player,
        game: game_pda(game_id).0,
        player_profile: player_profile_pda(&player),
        white_profile: player_profile_pda(&white),
        escrow_pda: escrow_pda(game_id),
        system_program: solana_system_interface::program::ID,
    }
    .to_account_metas(None);
    let data = xfchess_game::instruction::GlobalJoinGame { game_id }.data();
    Instruction {
        program_id: xfchess_game::ID,
        accounts,
        data,
    }
}

#[tokio::test]
async fn global_join_game_rejects_an_underfunded_session_vault() -> anyhow::Result<()> {
    let white = Pubkey::new_unique();
    let black = Pubkey::new_unique();
    let session_signer = Keypair::new();
    let game_id = 101u64;
    let wager_amount = 20_000_000u64;

    // Generous caps again â€” only the balance check may reject this.
    let (session_pda, mut session_account_data) =
        global_session_account(black, session_signer.pubkey(), 5_000_000_000, 1_000_000_000)?;
    session_account_data.lamports = 2_000_000;

    let (game_pda_key, game_account_data) = waiting_game_account(game_id, white, wager_amount)?;
    let (white_profile_pda, white_profile_data) = player_profile_account(white)?;
    let (black_profile_pda, black_profile_data) = player_profile_account(black)?;

    let mut ctx = start(vec![
        (session_pda, session_account_data),
        (game_pda_key, game_account_data),
        (white_profile_pda, white_profile_data),
        (black_profile_pda, black_profile_data),
    ])
    .await;

    let ix = global_join_game_ix(session_pda, session_signer.pubkey(), black, white, game_id);
    let err = send(&mut ctx, ix, &[&session_signer])
        .await
        .err()
        .context("an underfunded joiner vault must be rejected up front")?;

    assert_eq!(
        custom_code(&err),
        Some(ec(
            xfchess_game::errors::GameErrorCode::GlobalSessionVaultUnderfunded
        )),
        "expected GlobalSessionVaultUnderfunded, got {err:?}"
    );
    Ok(())
}

#[tokio::test]
async fn global_join_game_funds_wager_from_session_vault() -> anyhow::Result<()> {
    let white = Pubkey::new_unique();
    let black = Pubkey::new_unique();
    let session_signer = Keypair::new();
    let game_id = 99u64;
    let wager_amount = 20_000_000u64;

    let (session_pda, session_account_data) =
        global_session_account(black, session_signer.pubkey(), 5_000_000_000, 1_000_000_000)?;
    let (game_pda_key, game_account_data) = waiting_game_account(game_id, white, wager_amount)?;
    let (white_profile_pda, white_profile_data) = player_profile_account(white)?;
    let (black_profile_pda, black_profile_data) = player_profile_account(black)?;

    let mut ctx = start(vec![
        (session_pda, session_account_data),
        (game_pda_key, game_account_data),
        (white_profile_pda, white_profile_data),
        (black_profile_pda, black_profile_data),
    ])
    .await;

    let ix = global_join_game_ix(session_pda, session_signer.pubkey(), black, white, game_id);
    send(&mut ctx, ix, &[&session_signer])
        .await
        .context("global_join_game should succeed when the session vault PDA funds the wager")?;

    let game = fetch_game(&mut ctx, game_id).await?;
    assert_eq!(game.black, black);
    assert_eq!(game.status, GameStatus::Active);

    let escrow_balance = ctx
        .banks_client
        .get_balance(escrow_pda(game_id))
        .await
        .context("required operation value")?;
    assert_eq!(escrow_balance, wager_amount);

    let session_acc = ctx
        .banks_client
        .get_account(session_pda)
        .await
        .context("required operation value")?
        .context("session delegation account missing")?;
    let session = GlobalSessionDelegation::try_deserialize(&mut &session_acc.data[..])
        .context("required operation value")?;
    assert_eq!(session.total_spent, wager_amount);
    assert_eq!(session.games_remaining, 199);
    Ok(())
}
