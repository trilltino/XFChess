//! Test both serialized instruction orderings against the real program.
//! The losing instruction must leave state and lamports unchanged.

#[path = "common/agave.rs"]
pub mod agave;
use anyhow::Context as _;
pub mod common;

use crate::agave::ProgramTestContext;
use anchor_lang::{InstructionData, Space, ToAccountMetas};
use common::*;
use solana_sdk::{
    account::AccountSharedData, clock::Clock, instruction::Instruction, pubkey::Pubkey,
    signature::Keypair, signer::Signer,
};
use xfchess_game::errors::GameErrorCode;
use xfchess_game::state::{Game, GameResult, GameStatus, GameType, MatchType, PlayerProfile};

const WAGER: u64 = 50_000_000;
const PLAYER_FUNDS: u64 = 1_000_000_000;

fn escrow_pda(game_id: u64) -> Pubkey {
    Pubkey::find_program_address(&[b"escrow", &game_id.to_le_bytes()], &xfchess_game::ID).0
}

fn cancel_ix(game_id: u64, player: Pubkey, white: Pubkey, black: Pubkey) -> Instruction {
    let accounts = xfchess_game::__client_accounts_cancel_game::CancelGame {
        game: game_pda(game_id).0,
        escrow_pda: escrow_pda(game_id),
        player,
        white_authority: white,
        black_authority: black,
        system_program: solana_system_interface::program::ID,
    }
    .to_account_metas(None);
    // Same metas as the game client's `cancel_game_ix`: both authorities are
    // passed writable (the runtime demotes the unjoined default key itself).
    let accounts = accounts
        .into_iter()
        .map(|mut m| {
            if m.pubkey == black {
                m.is_writable = true;
            }
            m
        })
        .collect();
    Instruction {
        program_id: xfchess_game::ID,
        accounts,
        data: xfchess_game::instruction::CancelGame { game_id }.data(),
    }
}

fn join_ix(game_id: u64, player: Pubkey, white: Pubkey, fee_payer: Pubkey) -> Instruction {
    let accounts = xfchess_game::__client_accounts_join_game::JoinGame {
        game: game_pda(game_id).0,
        player_profile: profile_pda(&player).0,
        escrow_pda: escrow_pda(game_id),
        white_profile: profile_pda(&white).0,
        player,
        fee_payer,
        system_program: solana_system_interface::program::ID,
    }
    .to_account_metas(None);
    Instruction {
        program_id: xfchess_game::ID,
        accounts,
        data: xfchess_game::instruction::JoinGame { game_id }.data(),
    }
}

fn profile(authority: Pubkey) -> anyhow::Result<(Pubkey, solana_sdk::account::Account)> {
    Ok({
        let p = PlayerProfile {
            authority,
            ..Default::default()
        };
        (
            profile_pda(&authority).0,
            program_account(&p, 8 + PlayerProfile::INIT_SPACE)?,
        )
    })
}

#[allow(clippy::too_many_arguments)]
fn game(
    game_id: u64,
    white: Pubkey,
    black: Pubkey,
    fee_payer: Pubkey,
    status: GameStatus,
    move_count: u16,
    updated_at: i64,
) -> anyhow::Result<(Pubkey, solana_sdk::account::Account)> {
    Ok({
        let (pda, bump) = game_pda(game_id);
        let g = Game {
            game_id,
            white,
            black,
            status,
            last_move_timestamp: updated_at,
            fees_advanced: 0,
            fee_payer,
            result: GameResult::None,
            board_state: start_board(),
            move_count,
            halfmove_clock: 0,
            turn: move_count + 1,
            created_at: updated_at,
            updated_at,
            wager_amount: WAGER,
            wager_token: None,
            game_type: GameType::PvP,
            match_type: MatchType::Free,
            country_fee: 0,
            base_time_seconds: 300,
            increment_seconds: 0,
            bump,
            is_delegated: false,
            tournament_id: None,
            nonce: move_count as u64,
            draw_offered_by: None,
        };
        (pda, program_account(&g, 8 + Game::INIT_SPACE)?)
    })
}

async fn now(ctx: &mut ProgramTestContext) -> anyhow::Result<i64> {
    Ok({
        ctx.banks_client
            .get_sysvar::<Clock>()
            .await
            .context("required operation value")?
            .unix_timestamp
    })
}

async fn lamports(ctx: &mut ProgramTestContext, key: Pubkey) -> anyhow::Result<u64> {
    Ok({
        ctx.banks_client
            .get_account(key)
            .await
            .context("required operation value")?
            .map(|a| a.lamports)
            .unwrap_or(0)
    })
}

/// Advance to a fresh blockhash so a repeated identical instruction is a
/// distinct transaction rather than a deduplicated signature.
async fn next_blockhash(ctx: &mut ProgramTestContext) -> anyhow::Result<()> {
    ctx.last_blockhash = ctx
        .get_new_latest_blockhash()
        .await
        .context("required operation value")?;
    Ok(())
}

struct Table {
    ctx: ProgramTestContext,
    white: Keypair,
    black: Keypair,
}

/// A wagered game created by `white`, waiting for `black`; escrow funded with
/// white's stake. The test payer is the game's fee payer.
async fn waiting_table(game_id: u64) -> anyhow::Result<Table> {
    Ok({
        let white = Keypair::new();
        let black = Keypair::new();
        let mut ctx = start(vec![
            (white.pubkey(), system_account(PLAYER_FUNDS)),
            (black.pubkey(), system_account(PLAYER_FUNDS)),
            profile(white.pubkey())?,
            profile(black.pubkey())?,
            (escrow_pda(game_id), system_account(WAGER)),
        ])
        .await;
        let t = now(&mut ctx).await?;
        let (key, data) = game(
            game_id,
            white.pubkey(),
            Pubkey::default(),
            ctx.payer.pubkey(),
            GameStatus::WaitingForOpponent,
            0,
            t,
        )?;
        ctx.set_account(&key, &AccountSharedData::from(data));
        Table { ctx, white, black }
    })
}

#[tokio::test]
async fn cancel_wins_the_race_then_join_fails_and_joiner_keeps_funds() -> anyhow::Result<()> {
    let id = 70_001;
    let Table {
        mut ctx,
        white,
        black,
    } = waiting_table(id).await?;
    let white_before = lamports(&mut ctx, white.pubkey()).await?;
    let black_before = lamports(&mut ctx, black.pubkey()).await?;

    send(
        &mut ctx,
        cancel_ix(id, white.pubkey(), white.pubkey(), Pubkey::default()),
        &[&white],
    )
    .await
    .context("creator cancels an unjoined game")?;
    assert_eq!(
        fetch_game(&mut ctx, id).await?.status,
        GameStatus::Cancelled
    );
    assert_eq!(
        lamports(&mut ctx, white.pubkey()).await?,
        white_before + WAGER
    );
    assert_eq!(lamports(&mut ctx, escrow_pda(id)).await?, 0);

    let payer = ctx.payer.pubkey();
    let err = send(
        &mut ctx,
        join_ix(id, black.pubkey(), white.pubkey(), payer),
        &[&black],
    )
    .await
    .err()
    .context("join after cancellation must fail")?;
    assert_eq!(custom_code(&err), Some(ec(GameErrorCode::GameAlreadyFull)));
    assert_eq!(
        fetch_game(&mut ctx, id).await?.status,
        GameStatus::Cancelled
    );
    assert_eq!(
        lamports(&mut ctx, black.pubkey()).await?,
        black_before,
        "a rejected join moves no funds"
    );
    assert_eq!(lamports(&mut ctx, escrow_pda(id)).await?, 0);
    Ok(())
}

#[tokio::test]
async fn join_wins_the_race_then_creator_cancel_refunds_both_and_names_no_winner(
) -> anyhow::Result<()> {
    let id = 70_002;
    let Table {
        mut ctx,
        white,
        black,
    } = waiting_table(id).await?;
    let white_before = lamports(&mut ctx, white.pubkey()).await?;
    let black_before = lamports(&mut ctx, black.pubkey()).await?;

    let payer = ctx.payer.pubkey();
    send(
        &mut ctx,
        join_ix(id, black.pubkey(), white.pubkey(), payer),
        &[&black],
    )
    .await
    .context("join lands first")?;
    assert_eq!(lamports(&mut ctx, escrow_pda(id)).await?, 2 * WAGER);

    // The creator's cancel arrives second: before any move it is the
    // zero-move cancellation â€” both stakes back, no result.
    send(
        &mut ctx,
        cancel_ix(id, white.pubkey(), white.pubkey(), black.pubkey()),
        &[&white],
    )
    .await
    .context("zero-move cancellation after join")?;
    let g = fetch_game(&mut ctx, id).await?;
    assert_eq!(g.status, GameStatus::Cancelled);
    assert_eq!(g.result, GameResult::None);
    assert_eq!(
        lamports(&mut ctx, white.pubkey()).await?,
        white_before + WAGER
    );
    assert_eq!(lamports(&mut ctx, black.pubkey()).await?, black_before);
    assert_eq!(lamports(&mut ctx, escrow_pda(id)).await?, 0);
    Ok(())
}

#[tokio::test]
async fn duplicate_cancellation_is_safe_and_pays_nothing_twice() -> anyhow::Result<()> {
    let id = 70_003;
    let Table { mut ctx, white, .. } = waiting_table(id).await?;
    let cancel = cancel_ix(id, white.pubkey(), white.pubkey(), Pubkey::default());
    send(&mut ctx, cancel.clone(), &[&white])
        .await
        .context("required operation value")?;
    let after_first = lamports(&mut ctx, white.pubkey()).await?;

    // A retry after a lost acknowledgement.
    next_blockhash(&mut ctx).await?;
    send(&mut ctx, cancel, &[&white])
        .await
        .context("repeat cancellation is accepted (idempotent)")?;
    assert_eq!(lamports(&mut ctx, white.pubkey()).await?, after_first);
    assert_eq!(lamports(&mut ctx, escrow_pda(id)).await?, 0);
    assert_eq!(
        fetch_game(&mut ctx, id).await?.status,
        GameStatus::Cancelled
    );
    Ok(())
}

#[tokio::test]
async fn outsider_cannot_cancel_an_abandoned_active_game_but_a_player_can() -> anyhow::Result<()> {
    let id = 70_004;
    let white = Keypair::new();
    let black = Keypair::new();
    let outsider = Keypair::new();
    let mut ctx = start(vec![
        (white.pubkey(), system_account(PLAYER_FUNDS)),
        (black.pubkey(), system_account(PLAYER_FUNDS)),
        (outsider.pubkey(), system_account(PLAYER_FUNDS)),
        (escrow_pda(id), system_account(2 * WAGER)),
    ])
    .await;
    let t = now(&mut ctx).await?;
    let payer = ctx.payer.pubkey();
    // Moves played, idle for over the 24h abandonment window.
    let (key, data) = game(
        id,
        white.pubkey(),
        black.pubkey(),
        payer,
        GameStatus::Active,
        6,
        t - 3600 * 25,
    )?;
    ctx.set_account(&key, &AccountSharedData::from(data));

    let err = send(
        &mut ctx,
        cancel_ix(id, outsider.pubkey(), white.pubkey(), black.pubkey()),
        &[&outsider],
    )
    .await
    .err()
    .context("a non-player cannot cancel")?;
    assert_eq!(custom_code(&err), Some(ec(GameErrorCode::NotInGame)));
    assert_eq!(fetch_game(&mut ctx, id).await?.status, GameStatus::Active);
    assert_eq!(lamports(&mut ctx, escrow_pda(id)).await?, 2 * WAGER);

    send(
        &mut ctx,
        cancel_ix(id, black.pubkey(), white.pubkey(), black.pubkey()),
        &[&black],
    )
    .await
    .context("a player may cancel the abandoned game")?;
    assert_eq!(
        fetch_game(&mut ctx, id).await?.status,
        GameStatus::Cancelled
    );
    assert_eq!(lamports(&mut ctx, escrow_pda(id)).await?, 0);
    Ok(())
}

#[tokio::test]
async fn abandoned_cancel_is_refused_before_the_window() -> anyhow::Result<()> {
    let id = 70_005;
    let white = Keypair::new();
    let black = Keypair::new();
    let mut ctx = start(vec![
        (white.pubkey(), system_account(PLAYER_FUNDS)),
        (black.pubkey(), system_account(PLAYER_FUNDS)),
        (escrow_pda(id), system_account(2 * WAGER)),
    ])
    .await;
    let t = now(&mut ctx).await?;
    let payer = ctx.payer.pubkey();
    let (key, data) = game(
        id,
        white.pubkey(),
        black.pubkey(),
        payer,
        GameStatus::Active,
        6,
        t - 600,
    )?;
    ctx.set_account(&key, &AccountSharedData::from(data));

    // A disconnected-but-not-abandoned game cannot be cancelled out from
    // under the opponent: network loss alone never ends it.
    let err = send(
        &mut ctx,
        cancel_ix(id, white.pubkey(), white.pubkey(), black.pubkey()),
        &[&white],
    )
    .await
    .err()
    .context("cancellation before the abandonment window must fail")?;
    assert_eq!(custom_code(&err), Some(ec(GameErrorCode::GameNotExpired)));
    assert_eq!(fetch_game(&mut ctx, id).await?.status, GameStatus::Active);
    Ok(())
}

/// Active, timed game: black to move (`turn` even) and idle longer than its
/// whole 300s clock.
async fn idle_active_table(game_id: u64) -> anyhow::Result<Table> {
    Ok({
        let white = Keypair::new();
        let black = Keypair::new();
        let mut ctx = start(vec![]).await;
        let t = now(&mut ctx).await?;
        let (pda, bump) = game_pda(game_id);
        let g = Game {
            game_id,
            white: white.pubkey(),
            black: black.pubkey(),
            status: GameStatus::Active,
            last_move_timestamp: t - 400,
            fees_advanced: 0,
            fee_payer: white.pubkey(),
            result: GameResult::None,
            board_state: start_board(),
            move_count: 1,
            halfmove_clock: 0,
            turn: 2,
            created_at: t - 500,
            updated_at: t - 400,
            wager_amount: 0,
            wager_token: None,
            game_type: GameType::PvP,
            match_type: MatchType::Free,
            country_fee: 0,
            base_time_seconds: 300,
            increment_seconds: 0,
            bump,
            is_delegated: false,
            tournament_id: None,
            nonce: 1,
            draw_offered_by: None,
        };
        ctx.set_account(
            &pda,
            &AccountSharedData::from(program_account(&g, 8 + Game::INIT_SPACE)?),
        );
        Table { ctx, white, black }
    })
}

#[tokio::test]
async fn resignation_then_timeout_claim_keeps_the_single_result() -> anyhow::Result<()> {
    let id = 70_006;
    let Table {
        mut ctx,
        white,
        black,
    } = idle_active_table(id).await?;

    send(&mut ctx, resign_ix(id, white.pubkey()), &[&white])
        .await
        .context("white resigns")?;
    let g = fetch_game(&mut ctx, id).await?;
    assert_eq!(g.status, GameStatus::Finished);
    assert_eq!(g.result, GameResult::Winner(black.pubkey()));

    // Black's clock had also lapsed; the late timeout claim must not flip it.
    let payer = ctx.payer.pubkey();
    let err = send(&mut ctx, claim_timeout_ix(id, payer), &[])
        .await
        .err()
        .context("timeout after a terminal result must fail")?;
    assert_eq!(custom_code(&err), Some(ec(GameErrorCode::GameNotActive)));
    assert_eq!(
        fetch_game(&mut ctx, id).await?.result,
        GameResult::Winner(black.pubkey())
    );
    Ok(())
}

#[tokio::test]
async fn timeout_then_resignation_keeps_the_single_result() -> anyhow::Result<()> {
    let id = 70_007;
    let Table {
        mut ctx,
        white,
        black,
    } = idle_active_table(id).await?;

    let payer = ctx.payer.pubkey();
    send(&mut ctx, claim_timeout_ix(id, payer), &[])
        .await
        .context("black flagged")?;
    assert_eq!(
        fetch_game(&mut ctx, id).await?.result,
        GameResult::Winner(white.pubkey())
    );

    // The flagged player resigns late (e.g. reconnecting): no second result.
    assert!(send(&mut ctx, resign_ix(id, black.pubkey()), &[&black])
        .await
        .is_err());
    let g = fetch_game(&mut ctx, id).await?;
    assert_eq!(g.status, GameStatus::Finished);
    assert_eq!(g.result, GameResult::Winner(white.pubkey()));
    Ok(())
}

#[tokio::test]
async fn cancelled_game_cannot_be_resigned_or_timed_out() -> anyhow::Result<()> {
    let id = 70_008;
    let Table {
        mut ctx,
        white,
        black,
    } = waiting_table(id).await?;
    send(
        &mut ctx,
        cancel_ix(id, white.pubkey(), white.pubkey(), Pubkey::default()),
        &[&white],
    )
    .await
    .context("required operation value")?;

    assert!(send(&mut ctx, resign_ix(id, white.pubkey()), &[&white])
        .await
        .is_err());
    let payer = ctx.payer.pubkey();
    assert!(send(&mut ctx, claim_timeout_ix(id, payer), &[])
        .await
        .is_err());
    let _ = black;
    let g = fetch_game(&mut ctx, id).await?;
    assert_eq!(g.status, GameStatus::Cancelled);
    assert_eq!(g.result, GameResult::None);
    Ok(())
}

/// Anyone can send lamports to the escrow address; a sub-rent remainder after
/// the refund must be swept to the creator, not left to fail the cancel.
#[tokio::test]
async fn cancel_refunds_despite_a_sub_rent_escrow_donation() -> anyhow::Result<()> {
    let id = 70_009;
    let Table { mut ctx, white, .. } = waiting_table(id).await?;
    let white_before = lamports(&mut ctx, white.pubkey()).await?;
    let donation = 7u64;
    ctx.set_account(
        &escrow_pda(id),
        &AccountSharedData::from(system_account(WAGER + donation)),
    );
    send(
        &mut ctx,
        cancel_ix(id, white.pubkey(), white.pubkey(), Pubkey::default()),
        &[&white],
    )
    .await
    .context("cancel must not be blocked by dust")?;
    assert_eq!(lamports(&mut ctx, escrow_pda(id)).await?, 0);
    assert_eq!(
        lamports(&mut ctx, white.pubkey()).await?,
        white_before + WAGER + donation
    );
    Ok(())
}

fn finalize_ix(id: u64, white: Pubkey, black: Pubkey, fee_payer: Pubkey) -> Instruction {
    let accounts = xfchess_game::__client_accounts_end_game::EndGame {
        game: game_pda(id).0,
        white_profile: profile_pda(&white).0,
        black_profile: profile_pda(&black).0,
        white_authority: white,
        black_authority: black,
        escrow_pda: escrow_pda(id),
        treasury_vault: Pubkey::find_program_address(&[b"treasury_vault"], &xfchess_game::ID).0,
        fee_payer,
        system_program: solana_system_interface::program::ID,
    }
    .to_account_metas(None);
    Instruction {
        program_id: xfchess_game::ID,
        accounts,
        data: xfchess_game::instruction::FinalizeGame { game_id: id }.data(),
    }
}

fn rank(status: GameStatus) -> u8 {
    match status {
        GameStatus::Pending | GameStatus::WaitingForOpponent => 0,
        GameStatus::Active | GameStatus::Inactive | GameStatus::Disputed => 1,
        GameStatus::Finished => 2,
        GameStatus::Settled | GameStatus::Cancelled | GameStatus::Expired => 3,
    }
}

/// Model-based check: random sequences of lifecycle operations, repeats and
/// replays included. After every step (accepted or rejected) lamports are never
/// created, a rejected step changes nothing, and the Game never moves backwards.
#[tokio::test]
async fn random_operation_sequences_conserve_lamports_and_never_regress() -> anyhow::Result<()> {
    use xfchess_game::state::GameStatus as S;
    let base: u64 = std::env::var("LIFECYCLE_SEED")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(7);
    let treasury = Pubkey::find_program_address(&[b"treasury_vault"], &xfchess_game::ID).0;
    let mut total_accepted = 0u32;
    for run in 0..16u64 {
        let seed = base.wrapping_mul(0x9E37_79B9).wrapping_add(run);
        let id = 71_000 + run;
        let Table {
            mut ctx,
            white,
            black,
        } = waiting_table(id).await?;
        ctx.set_account(
            &treasury,
            &AccountSharedData::from(system_account(2_000_000)),
        );
        let payer = ctx.payer.pubkey();
        let tracked = [
            payer,
            white.pubkey(),
            black.pubkey(),
            escrow_pda(id),
            game_pda(id).0,
            treasury,
        ];
        let mut state = seed | 1;
        let mut next_random = move || {
            state ^= state >> 12;
            state ^= state << 25;
            state ^= state >> 27;
            state.wrapping_mul(0x2545_F491_4F6C_DD1D)
        };
        let mut best_rank = 0u8;
        for step in 0..14u32 {
            let op = next_random() % 8;
            let (label, ix, signer): (&str, Instruction, Option<&Keypair>) = match op {
                0 | 1 => (
                    "join",
                    join_ix(id, black.pubkey(), white.pubkey(), payer),
                    Some(&black),
                ),
                2 => (
                    "cancel by white",
                    cancel_ix(id, white.pubkey(), white.pubkey(), black.pubkey()),
                    Some(&white),
                ),
                3 => (
                    "cancel by black",
                    cancel_ix(id, black.pubkey(), white.pubkey(), black.pubkey()),
                    Some(&black),
                ),
                4 => (
                    "resign by white",
                    resign_ix(id, white.pubkey()),
                    Some(&white),
                ),
                5 => (
                    "resign by black",
                    resign_ix(id, black.pubkey()),
                    Some(&black),
                ),
                6 => (
                    "claim timeout",
                    claim_timeout_ix(id, white.pubkey()),
                    Some(&white),
                ),
                _ => (
                    "finalize",
                    finalize_ix(id, white.pubkey(), black.pubkey(), payer),
                    None,
                ),
            };
            let mut before = Vec::new();
            for key in tracked {
                before.push(ctx.banks_client.get_account(key).await?);
            }
            let sum_before: u64 = before
                .iter()
                .map(|account| account.as_ref().map_or(0, |a| a.lamports))
                .sum();
            next_blockhash(&mut ctx).await?;
            let signers: Vec<&Keypair> = signer.into_iter().collect();
            let result = send(&mut ctx, ix, &signers).await;
            let mut after = Vec::new();
            for key in tracked {
                after.push(ctx.banks_client.get_account(key).await?);
            }
            let sum_after: u64 = after
                .iter()
                .map(|account| account.as_ref().map_or(0, |a| a.lamports))
                .sum();
            let context = format!("seed {seed} step {step} {label}");
            assert!(sum_after <= sum_before, "{context}: lamports were created");
            assert!(
                sum_before - sum_after <= 10_000,
                "{context}: more than the transaction fee left the tracked set"
            );
            match result {
                Ok(()) => total_accepted += 1,
                Err(_) => {
                    // Only the fee payer's balance may move on a rejected step.
                    assert_eq!(
                        before[1..],
                        after[1..],
                        "{context}: rejected step mutated state"
                    );
                }
            }
            if let Some(account) = &after[4] {
                if account.owner == xfchess_game::ID && !account.data.is_empty() {
                    let status = fetch_game(&mut ctx, id).await?.status;
                    assert!(
                        rank(status) >= best_rank,
                        "{context}: status regressed to {status:?}"
                    );
                    best_rank = rank(status);
                    if matches!(status, S::Settled) {
                        best_rank = 3;
                    }
                }
            } else {
                best_rank = 3;
            }
        }
    }
    assert!(total_accepted > 16, "the model never made progress");
    Ok(())
}
