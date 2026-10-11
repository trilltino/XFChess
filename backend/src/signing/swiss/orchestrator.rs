//! Routes settled Swiss games into [`SwissService`] and mirrors the result onto the
//! chain.
//!
//! It holds no state of its own. A Swiss game id encodes its `(round, board)`
//! ([`TournamentStore::swiss_game_id`]); chain progress comes from the store
//! (`chain_boards_recorded`, `chain_completed`) and the on-chain `Tournament` account
//! (`current_round`). So a restart, a dropped event or a failed transaction is
//! repaired by the next reconcile pass, which also runs on a timer.

use crate::signing::solana::instructions::{
    SWISS_BLACK_WIN, SWISS_BYE, SWISS_DRAW, SWISS_WHITE_WIN,
};
use crate::signing::solana::tournament_account::{
    fetch_swiss_progress, TOURNAMENT_STATUS_ACTIVE, TOURNAMENT_STATUS_COMPLETED,
};
use crate::signing::storage::tournament::{TournamentFormat, TournamentStatus, TournamentStore};
use crate::signing::swiss::service::{SwissService, SwissServiceError};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use swiss_pairing::MatchResult;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

/// Anchor code for `GameErrorCode::BoardAlreadyRecorded` (6000 + variant index): the
/// board landed on an earlier attempt whose confirmation was lost.
const BOARD_ALREADY_RECORDED: &str = "custom program error: 0x17cf";

const RECONCILE_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Debug, Clone)]
pub enum OrchestratorEvent {
    /// A tournament game settled on-chain. `white`/`black` are the on-chain Game's
    /// players, checked against the pairing the game id points at.
    GameEnded {
        tournament_id: u64,
        game_id: u64,
        white: String,
        black: String,
        result: MatchResult,
    },
    /// Off-chain state changed (admin result, forfeit, new round); mirror it now.
    Reconcile { tournament_id: u64 },
}

#[derive(Clone)]
pub struct SwissOnChainConfig {
    pub program_id: String,
    pub rpc_url: String,
    /// Must be the tournament's on-chain `authority`.
    pub authority: Arc<Keypair>,
}

pub struct SwissOrchestrator {
    store: TournamentStore,
    swiss: Arc<SwissService>,
    event_rx: mpsc::Receiver<OrchestratorEvent>,
    on_chain: Option<SwissOnChainConfig>,
}

impl SwissOrchestrator {
    pub fn new(
        store: TournamentStore,
        swiss: Arc<SwissService>,
        on_chain: Option<SwissOnChainConfig>,
    ) -> (Self, mpsc::Sender<OrchestratorEvent>) {
        let (tx, rx) = mpsc::channel(256);
        let orchestrator = Self {
            store,
            swiss,
            event_rx: rx,
            on_chain,
        };
        (orchestrator, tx)
    }

    pub async fn run(mut self) {
        info!("[orchestrator] Swiss orchestrator started");
        let mut tick = tokio::time::interval(RECONCILE_INTERVAL);
        loop {
            tokio::select! {
                event = self.event_rx.recv() => match event {
                    Some(OrchestratorEvent::GameEnded { tournament_id, game_id, white, black, result }) => {
                        self.handle_game_ended(tournament_id, game_id, &white, &black, result).await;
                        self.reconcile(tournament_id).await;
                    }
                    Some(OrchestratorEvent::Reconcile { tournament_id }) => {
                        self.reconcile(tournament_id).await;
                    }
                    None => break,
                },
                _ = tick.tick() => self.reconcile_all().await,
            }
        }
        info!("[orchestrator] Swiss orchestrator stopped");
    }

    async fn handle_game_ended(
        &self,
        tournament_id: u64,
        game_id: u64,
        white: &str,
        black: &str,
        result: MatchResult,
    ) {
        let Some((round, board)) = TournamentStore::swiss_board_for_game_id(tournament_id, game_id)
        else {
            warn!(
                "[orchestrator] game {} is not a Swiss board of tournament {}",
                game_id, tournament_id
            );
            return;
        };

        let Some(tournament) = self.store.get(tournament_id).await else {
            return;
        };
        let pairing = tournament
            .swiss_data
            .as_ref()
            .and_then(|sd| sd.round(round))
            .and_then(|r| r.pairings.iter().find(|p| p.board == board));
        let Some(pairing) = pairing else {
            warn!(
                "[orchestrator] game {} maps to unknown round {} board {} in tournament {}",
                game_id, round, board, tournament_id
            );
            return;
        };
        if pairing.white != white || pairing.black != black {
            error!(
                "[orchestrator] game {} players {} / {} do not match pairing {} / {} \
                 (tournament {} round {} board {}) — result ignored",
                game_id, white, black, pairing.white, pairing.black, tournament_id, round, board
            );
            return;
        }

        match self
            .swiss
            .record_result(tournament_id, round, board, result)
            .await
        {
            Ok(_) => info!(
                "[orchestrator] game {} recorded: tournament {} round {} board {} {:?}",
                game_id, tournament_id, round, board, result
            ),
            Err(SwissServiceError::ResultAlreadyRecorded) => {}
            Err(e) => error!(
                "[orchestrator] recording game {} (tournament {} round {} board {}) failed: {}",
                game_id, tournament_id, round, board, e
            ),
        }
    }

    async fn reconcile_all(&self) {
        if self.on_chain.is_none() {
            return;
        }
        for t in self.store.list().await {
            let pending = matches!(t.format, TournamentFormat::Swiss { .. })
                && matches!(
                    t.status,
                    TournamentStatus::Active | TournamentStatus::Completed
                )
                && t.swiss_data.as_ref().is_some_and(|sd| !sd.chain_completed);
            if pending {
                self.reconcile(t.tournament_id).await;
            }
        }
    }

    /// Brings the on-chain tournament up to the store: record every decided board of
    /// the on-chain round, advance once all are in, and complete after the last round.
    async fn reconcile(&self, tournament_id: u64) {
        let Some(cfg) = self.on_chain.clone() else {
            return;
        };
        let Some(tournament) = self.store.get(tournament_id).await else {
            return;
        };
        let Some(sd) = tournament.swiss_data.clone() else {
            return;
        };
        if sd.chain_completed {
            return;
        }
        let Ok(program_id) = Pubkey::from_str(&cfg.program_id) else {
            error!("[orchestrator] invalid program ID {}", cfg.program_id);
            return;
        };
        let max_players = tournament.max_players;
        let off_chain_completed = tournament.status == TournamentStatus::Completed;
        let places: Vec<String> = sd
            .standings
            .iter()
            .take(10)
            .map(|s| s.player_id.clone())
            .collect();

        let outcome = tokio::task::spawn_blocking(move || {
            mirror_to_chain(
                &cfg,
                &program_id,
                tournament_id,
                max_players,
                &sd,
                off_chain_completed,
                &places,
            )
        })
        .await;

        let progress = match outcome {
            Ok(progress) => progress,
            Err(e) => {
                error!(
                    "[orchestrator] reconcile task for tournament {tournament_id} panicked: {e}"
                );
                return;
            }
        };
        if let Some(e) = &progress.error {
            warn!("[orchestrator] tournament {tournament_id} chain reconcile stopped: {e}");
        }
        if progress.recorded.is_empty() && !progress.completed {
            return;
        }
        self.store
            .update(tournament_id, |t| {
                if let Some(sd) = t.swiss_data.as_mut() {
                    for board in &progress.recorded {
                        if !sd.chain_boards_recorded.contains(board) {
                            sd.chain_boards_recorded.push(*board);
                        }
                    }
                    sd.chain_completed |= progress.completed;
                }
            })
            .await;
        if progress.completed {
            info!("[orchestrator] tournament {tournament_id} completed on-chain");
        }
    }
}

#[derive(Default)]
struct ChainProgress {
    /// `(round, board)` pairs (backend numbering) newly confirmed on-chain.
    recorded: Vec<(u8, u16)>,
    completed: bool,
    error: Option<String>,
}

fn chain_result(result: MatchResult) -> Option<u8> {
    match result {
        MatchResult::WhiteWin | MatchResult::ForfeitWhiteWin => Some(SWISS_WHITE_WIN),
        MatchResult::BlackWin | MatchResult::ForfeitBlackWin => Some(SWISS_BLACK_WIN),
        MatchResult::Draw => Some(SWISS_DRAW),
        MatchResult::Bye => None,
    }
}

/// Blocking body of [`SwissOrchestrator::reconcile`]. Backend rounds and boards are
/// 1-based; the program's are 0-based.
fn mirror_to_chain(
    cfg: &SwissOnChainConfig,
    program_id: &Pubkey,
    tournament_id: u64,
    max_players: u16,
    sd: &crate::signing::storage::tournament::SwissStorageData,
    off_chain_completed: bool,
    places: &[String],
) -> ChainProgress {
    use crate::signing::solana::{
        advance_round_ix, complete_swiss_tournament_ix, make_rpc, record_swiss_result_ix,
        sign_and_submit,
    };

    let mut progress = ChainProgress::default();
    let rpc = make_rpc(&cfg.rpc_url);
    let authority = cfg.authority.as_ref();
    let mut chain = match fetch_swiss_progress(&rpc, program_id, tournament_id) {
        Ok(chain) => chain,
        Err(e) => {
            progress.error = Some(e);
            return progress;
        }
    };
    if chain.status == TOURNAMENT_STATUS_COMPLETED {
        progress.completed = true;
        return progress;
    }
    if chain.status != TOURNAMENT_STATUS_ACTIVE {
        // Not started on-chain yet; the scheduler's start will land first.
        return progress;
    }

    while chain.current_round < chain.total_rounds {
        let round = chain.current_round + 1;
        let Some(paired) = sd.round(round) else {
            return progress;
        };

        // Board slots for this round: decided pairings at their own board number,
        // then each bye as a pseudo-board after the last real one.
        let mut slots: Vec<(u16, u8, &str, &str)> = paired
            .pairings
            .iter()
            .filter_map(|p| {
                let (_, _, result) = sd
                    .results
                    .iter()
                    .find(|(r, b, _)| *r == round && *b == p.board)?;
                Some((
                    p.board,
                    chain_result(*result)?,
                    p.white.as_str(),
                    p.black.as_str(),
                ))
            })
            .collect();
        let first_bye_board = paired.pairings.len() as u16 + 1;
        slots.extend(paired.byes.iter().enumerate().map(|(i, bye)| {
            (
                first_bye_board + i as u16,
                SWISS_BYE,
                bye.as_str(),
                bye.as_str(),
            )
        }));
        let total_slots = paired.pairings.len() + paired.byes.len();

        for (board, variant, white, black) in slots {
            let key = (round, board);
            if sd.chain_boards_recorded.contains(&key) {
                continue;
            }
            let (Ok(white), Ok(black)) = (Pubkey::from_str(white), Pubkey::from_str(black)) else {
                progress.error = Some(format!("round {round} board {board}: non-pubkey player id"));
                return progress;
            };
            let ix = record_swiss_result_ix(
                program_id,
                tournament_id,
                max_players,
                chain.current_round,
                board - 1,
                variant,
                &white,
                &black,
                &authority.pubkey(),
            );
            match sign_and_submit(&rpc, authority, &[ix]) {
                Ok(_) => progress.recorded.push(key),
                Err(e)
                    if e.to_string()
                        .to_lowercase()
                        .contains(BOARD_ALREADY_RECORDED) =>
                {
                    progress.recorded.push(key)
                }
                Err(e) => {
                    progress.error = Some(format!(
                        "record_swiss_result round {round} board {board}: {e}"
                    ));
                    return progress;
                }
            }
        }

        let all_recorded = (1..=total_slots as u16).all(|board| {
            sd.chain_boards_recorded.contains(&(round, board))
                || progress.recorded.contains(&(round, board))
        });
        if !sd.round_complete(round) || !all_recorded {
            return progress;
        }

        let ix = advance_round_ix(
            program_id,
            tournament_id,
            total_slots as u16,
            &authority.pubkey(),
        );
        if let Err(e) = sign_and_submit(&rpc, authority, &[ix]) {
            progress.error = Some(format!("advance_round from round {round}: {e}"));
            return progress;
        }
        chain.current_round += 1;
    }

    if !off_chain_completed {
        return progress;
    }
    let places: Result<Vec<Pubkey>, _> = places.iter().map(|p| Pubkey::from_str(p)).collect();
    let Ok(places) = places else {
        progress.error = Some("final standings contain a non-pubkey player id".into());
        return progress;
    };
    let ix = complete_swiss_tournament_ix(
        program_id,
        tournament_id,
        max_players,
        &places,
        &authority.pubkey(),
    );
    match sign_and_submit(&rpc, authority, &[ix]) {
        Ok(_) => progress.completed = true,
        Err(e) => progress.error = Some(format!("complete_swiss_tournament: {e}")),
    }
    progress
}

pub fn spawn_orchestrator(
    store: TournamentStore,
    swiss: Arc<SwissService>,
    on_chain: Option<SwissOnChainConfig>,
) -> mpsc::Sender<OrchestratorEvent> {
    let (orchestrator, tx) = SwissOrchestrator::new(store, swiss, on_chain);
    tokio::spawn(orchestrator.run());
    tx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_scored_result_maps_to_a_chain_variant() {
        assert_eq!(chain_result(MatchResult::WhiteWin), Some(SWISS_WHITE_WIN));
        assert_eq!(
            chain_result(MatchResult::ForfeitWhiteWin),
            Some(SWISS_WHITE_WIN)
        );
        assert_eq!(chain_result(MatchResult::BlackWin), Some(SWISS_BLACK_WIN));
        assert_eq!(
            chain_result(MatchResult::ForfeitBlackWin),
            Some(SWISS_BLACK_WIN)
        );
        assert_eq!(chain_result(MatchResult::Draw), Some(SWISS_DRAW));
        assert_eq!(chain_result(MatchResult::Bye), None);
    }
}
