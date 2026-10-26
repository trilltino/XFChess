//! P2: every instruction is only reachable from its documented Game statuses.
//! For each status outside an instruction's allowed set the call must fail and
//! leave the Game account byte-for-byte unchanged. The allowed sets mirror the
//! table in docs/security/state-machine-20261025.md.
#[path = "common/agave.rs"]
pub mod agave;
use anyhow::Context as _;
pub mod common;

use common::*;
use solana_sdk::instruction::Instruction;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};
use xfchess_game::state::GameStatus;

const GAME_ID: u64 = 42;
const ALL_STATUSES: [GameStatus; 9] = [
    GameStatus::Pending,
    GameStatus::WaitingForOpponent,
    GameStatus::Active,
    GameStatus::Inactive,
    GameStatus::Disputed,
    GameStatus::Finished,
    GameStatus::Settled,
    GameStatus::Expired,
    GameStatus::Cancelled,
];

#[derive(Clone, Copy)]
enum Who {
    White,
    Black,
    Session,
}

struct Case {
    name: &'static str,
    allowed: &'static [GameStatus],
    /// Run the call from an allowed status and require success, proving the
    /// signer and accounts are right so the rejections are about status.
    control: bool,
}

async fn assert_only_allowed_statuses_run(
    case: &Case,
    build: impl Fn(&Pubkey, &Pubkey, &Keypair) -> (Instruction, Who),
) -> anyhow::Result<()> {
    let white = Keypair::new();
    let black = Keypair::new();
    let session = Keypair::new();
    for status in ALL_STATUSES {
        let allowed = case.allowed.contains(&status);
        if allowed && !case.control {
            continue;
        }
        let (game_key, game_acc) = game_account_with_delegation(
            GAME_ID,
            white.pubkey(),
            black.pubkey(),
            start_board(),
            1,
            0,
            status,
            false,
        )?;
        let mut ctx = start(vec![
            (game_key, game_acc.clone()),
            session_account(GAME_ID, white.pubkey(), session.pubkey(), i64::MAX, true)?,
            (white.pubkey(), system_account(1_000_000_000)),
            (black.pubkey(), system_account(1_000_000_000)),
        ])
        .await;
        let (ix, who) = build(&white.pubkey(), &black.pubkey(), &session);
        let signers: Vec<&Keypair> = vec![match who {
            Who::White => &white,
            Who::Black => &black,
            Who::Session => &session,
        }];
        let result = send(&mut ctx, ix, &signers).await;
        if allowed {
            result.with_context(|| format!("{} control from {status:?}", case.name))?;
            continue;
        }
        assert!(
            result.is_err(),
            "{} must be rejected from {status:?}",
            case.name
        );
        let after = ctx
            .banks_client
            .get_account(game_key)
            .await?
            .context("game must remain")?;
        assert_eq!(after, game_acc, "{} mutated a {status:?} game", case.name);
    }
    Ok(())
}

#[tokio::test]
async fn resign_runs_only_on_active_games() -> anyhow::Result<()> {
    assert_only_allowed_statuses_run(
        &Case {
            name: "resign",
            allowed: &[GameStatus::Active],
            control: true,
        },
        |white, _, _| (resign_ix(GAME_ID, *white), Who::White),
    )
    .await
}

fn draw_ix(game_id: u64, player: Pubkey, accept: bool) -> Instruction {
    use anchor_lang::{InstructionData, ToAccountMetas};
    let (accounts, data) = if accept {
        (
            xfchess_game::__client_accounts_accept_draw::AcceptDraw {
                game: game_pda(game_id).0,
                player,
            }
            .to_account_metas(None),
            xfchess_game::instruction::AcceptDraw { game_id }.data(),
        )
    } else {
        (
            xfchess_game::__client_accounts_offer_draw::OfferDraw {
                game: game_pda(game_id).0,
                player,
            }
            .to_account_metas(None),
            xfchess_game::instruction::OfferDraw { game_id }.data(),
        )
    };
    Instruction {
        program_id: xfchess_game::ID,
        accounts,
        data,
    }
}

#[tokio::test]
async fn draw_offers_and_acceptances_run_only_on_active_games() -> anyhow::Result<()> {
    for accept in [false, true] {
        assert_only_allowed_statuses_run(
            &Case {
                name: if accept { "accept_draw" } else { "offer_draw" },
                allowed: &[GameStatus::Active],
                control: !accept,
            },
            move |white, _, _| (draw_ix(GAME_ID, *white, accept), Who::White),
        )
        .await?;
    }
    Ok(())
}

#[tokio::test]
async fn claim_timeout_runs_only_on_active_games() -> anyhow::Result<()> {
    assert_only_allowed_statuses_run(
        &Case {
            name: "claim_timeout",
            allowed: &[GameStatus::Active],
            control: false,
        },
        |_, black, _| (claim_timeout_ix(GAME_ID, *black), Who::Black),
    )
    .await
}

#[tokio::test]
async fn record_move_runs_only_on_active_games() -> anyhow::Result<()> {
    let board = start_board();
    let mv = uci("e2e4");
    let next = apply(&board, &mv)?;
    assert_only_allowed_statuses_run(
        &Case {
            name: "record_move",
            allowed: &[GameStatus::Active],
            control: true,
        },
        move |white, _, session| {
            (
                record_move_ix(GAME_ID, white, &session.pubkey(), mv, next, 1, Some(0)),
                Who::Session,
            )
        },
    )
    .await
}
