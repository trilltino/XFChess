//! The puzzle play loop: load a puzzle, set up the board, send each player move to
//! the server, and play its replies.
//!
//! [`PuzzlePhase`] is a resource field rather than a `SubStates` because loading
//! starts in the main menu and finishes in `InGame` (and "next puzzle" reloads
//! from inside `InGame`), so it does not nest under a single parent state.

use bevy::prelude::*;

use super::api::{self, MoveOutcome, PuzzleData, PuzzleRequest};
use super::uci::{coords_to_uci, square, uci_to_event};
use crate::core::net_task::{Pending, Poll};
use crate::core::states::{GameMode, GameState};
use crate::engine::board_state::ChessEngine;
use crate::game::ai::{AiMode, ChessAIResource};
use crate::game::events::{MoveMadeEvent, NetworkMoveEvent};
use crate::rendering::pieces::{PieceColor, PiecesSpawned};

/// Asks for a new puzzle; from the menu hub, or "Next" in the HUD.
#[derive(Message, Clone, Debug)]
pub struct StartPuzzle(pub PuzzleRequest);

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum PuzzlePhase {
    #[default]
    Idle,
    /// Waiting for the server to pick a puzzle.
    Loading,
    /// Loaded; waiting for the `InGame` board to spawn.
    AwaitingBoard,
    /// The player's move.
    Playing,
    /// The server is checking the player's move.
    Checking,
    /// Solved or failed; [`PuzzleSession::result`] has the outcome.
    Done,
}

/// The current puzzle and where the player is in it.
#[derive(Resource, Default)]
pub struct PuzzleSession {
    /// The request that produced the current puzzle (reused by "Next").
    pub request: Option<PuzzleRequest>,
    pub active: Option<PuzzleData>,
    pub phase: PuzzlePhase,
    /// Solution moves found so far.
    pub found: usize,
    /// What to tell the player right now.
    pub status: String,
    pub result: Option<MoveOutcome>,
    /// The square the next solution move starts from, once a hint was asked for.
    pub hint: Option<String>,
    /// A hint was used, so the round is unrated.
    pub hinted: bool,
    loading: Option<Pending<Result<PuzzleData, String>>>,
    checking: Option<Pending<Result<MoveOutcome, String>>>,
    hinting: Option<Pending<Result<String, String>>>,
}

impl PuzzleSession {
    pub fn is_busy(&self) -> bool {
        self.loading.is_some()
    }

    /// Asks the server which piece to move. Ignored unless it is the player's move.
    pub fn request_hint(&mut self) {
        let Some(nonce) = self.active.as_ref().map(|p| p.nonce.clone()) else {
            return;
        };
        if self.phase != PuzzlePhase::Playing || self.hinting.is_some() {
            return;
        }
        self.hinting = Some(Pending::spawn("puzzle-hint", move || {
            api::request_hint(&nonce)
        }));
    }

    fn fail(&mut self, message: String) {
        warn!("[puzzle] {message}");
        self.status = message;
    }
}

/// What the board shows while a puzzle is loaded; read when `InGame` spawns
/// pieces and assigns players.
#[derive(Resource)]
pub struct PuzzleBoard {
    pub active: bool,
    /// The position before the opponent's setup move.
    pub fen: String,
    /// The side the player solves for.
    pub player_color: PieceColor,
}

impl Default for PuzzleBoard {
    fn default() -> Self {
        Self {
            active: false,
            fen: String::new(),
            player_color: PieceColor::White,
        }
    }
}

pub(super) fn start_puzzle(
    mut requests: MessageReader<StartPuzzle>,
    mut session: ResMut<PuzzleSession>,
) {
    let Some(StartPuzzle(request)) = requests.read().last().cloned() else {
        return;
    };
    if session.is_busy() {
        return;
    }
    if request.wallet.trim().is_empty() {
        let need = if request.kind.needs_wallet() {
            "connect a wallet first"
        } else {
            "no local identity available"
        };
        session.fail(need.into());
        return;
    }
    let fetch = request.clone();
    session.loading = Some(Pending::spawn("puzzle-load", move || {
        api::fetch_puzzle(&fetch)
    }));
    session.request = Some(request);
    session.phase = PuzzlePhase::Loading;
    session.status = "Loading puzzle…".into();
}

pub(super) fn poll_loading(
    mut session: ResMut<PuzzleSession>,
    mut board: ResMut<PuzzleBoard>,
    mut game_mode: ResMut<GameMode>,
    mut ai_config: ResMut<ChessAIResource>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    let Some(pending) = session.loading.as_ref() else {
        return;
    };
    let puzzle = match pending.poll() {
        Poll::Running => return,
        Poll::Ready(result) => result,
        Poll::Lost => Err("puzzle request failed".into()),
    };
    session.loading = None;
    // A failed load leaves whatever was on screen (menu or finished puzzle) alone.
    let stay = if session.active.is_some() {
        PuzzlePhase::Done
    } else {
        PuzzlePhase::Idle
    };
    let puzzle = match puzzle {
        Ok(p) if p.exhausted => {
            session.phase = stay;
            session.fail("No more puzzles match — try another theme or difficulty".into());
            return;
        }
        Ok(p) if p.already_attempted => {
            session.phase = stay;
            session.fail("You've already played today's puzzle".into());
            return;
        }
        Ok(p) => p,
        Err(e) => {
            session.phase = stay;
            session.fail(e);
            return;
        }
    };

    info!(
        "[puzzle] loaded {} ({}, plays {}, {} moves, source {})",
        puzzle.id, puzzle.rating, puzzle.color, puzzle.solution_len, puzzle.source
    );
    *board = PuzzleBoard {
        active: true,
        fen: puzzle.fen.clone(),
        player_color: if puzzle.color == "black" {
            PieceColor::Black
        } else {
            PieceColor::White
        },
    };
    // The server plays the other side, so the local AI must stay idle.
    *game_mode = GameMode::Puzzle;
    ai_config.mode = AiMode::Multiplayer;
    session.active = Some(puzzle);
    session.found = 0;
    session.result = None;
    session.hint = None;
    session.hinted = false;
    session.checking = None;
    session.hinting = None;
    session.phase = PuzzlePhase::AwaitingBoard;
    session.status = "Setting up the board…".into();
    // Re-enters InGame when already there, which rebuilds the board for "Next".
    next_state.set(GameState::InGame);
}

/// Once the board has spawned: load the puzzle position and play the
/// opponent's setup move, so the player faces the tactic.
pub(super) fn set_up_board(
    mut session: ResMut<PuzzleSession>,
    board: Res<PuzzleBoard>,
    pieces_spawned: Res<PiecesSpawned>,
    mut engine: ResMut<ChessEngine>,
    mut moves: MessageWriter<NetworkMoveEvent>,
) {
    if session.phase != PuzzlePhase::AwaitingBoard || !board.active || !pieces_spawned.spawned {
        return;
    }
    if let Err(e) = engine.set_from_fen(&board.fen) {
        session.phase = PuzzlePhase::Done;
        session.fail(format!("bad puzzle position: {e}"));
        return;
    }
    engine.rebuild_legal_move_cache();

    let setup = session
        .active
        .as_ref()
        .and_then(|p| uci_to_event(&p.setup_move));
    match setup {
        Some(event) => {
            moves.write(event);
            session.phase = PuzzlePhase::Playing;
            session.status = "Your move".into();
        }
        None => {
            session.phase = PuzzlePhase::Done;
            session.fail("bad puzzle setup move".into());
        }
    }
}

/// Sends the player's own move to the server for checking.
pub(super) fn submit_player_move(
    mut session: ResMut<PuzzleSession>,
    mut made: MessageReader<MoveMadeEvent>,
) {
    let mut chosen = None;
    for event in made.read().filter(|e| !e.remote) {
        if session.phase == PuzzlePhase::Playing {
            chosen = Some(coords_to_uci(event.from, event.to, event.promotion));
        }
    }
    let Some(uci) = chosen else { return };
    let Some(nonce) = session.active.as_ref().map(|p| p.nonce.clone()) else {
        return;
    };
    session.checking = Some(Pending::spawn("puzzle-move", move || {
        api::submit_move(&nonce, &uci)
    }));
    session.phase = PuzzlePhase::Checking;
    session.hint = None;
    session.status = "Checking…".into();
}

pub(super) fn poll_move(
    mut session: ResMut<PuzzleSession>,
    mut moves: MessageWriter<NetworkMoveEvent>,
) {
    let Some(pending) = session.checking.as_ref() else {
        return;
    };
    let outcome = match pending.poll() {
        Poll::Running => return,
        Poll::Ready(result) => result,
        Poll::Lost => Err("move check failed".into()),
    };
    session.checking = None;
    let outcome = match outcome {
        Ok(o) => o,
        Err(e) => {
            session.phase = PuzzlePhase::Done;
            session.fail(e);
            return;
        }
    };

    if outcome.correct {
        session.found += 1;
    }
    if outcome.done || !outcome.correct {
        session.phase = PuzzlePhase::Done;
        session.status = if outcome.win {
            "Solved!"
        } else {
            "Not the move"
        }
        .into();
        if let Some(sig) = &outcome.payout_sig {
            info!("[puzzle] reward paid: {sig}");
        }
        session.result = Some(outcome);
        return;
    }
    match outcome.reply.as_deref().and_then(uci_to_event) {
        Some(reply) => {
            moves.write(reply);
            session.phase = PuzzlePhase::Playing;
            session.status = "Correct — keep going".into();
        }
        None => {
            session.phase = PuzzlePhase::Done;
            session.fail("the server sent no reply move".into());
        }
    }
}

pub(super) fn poll_hint(mut session: ResMut<PuzzleSession>) {
    let Some(pending) = session.hinting.as_ref() else {
        return;
    };
    let hint = match pending.poll() {
        Poll::Running => return,
        Poll::Ready(result) => result,
        Poll::Lost => Err("hint request failed".into()),
    };
    session.hinting = None;
    match hint {
        Ok(from) if square(&from).is_some() => {
            session.hinted = true;
            session.status = format!("Hint: move the piece on {from}");
            session.hint = Some(from);
        }
        Ok(from) => session.fail(format!("unexpected hint square {from:?}")),
        Err(e) => session.fail(e),
    }
}

/// Leaving gameplay (not just pausing, and not the "Next" reload) ends the puzzle.
pub(super) fn end_on_exit(mut session: ResMut<PuzzleSession>, mut board: ResMut<PuzzleBoard>) {
    *session = PuzzleSession::default();
    board.active = false;
}
