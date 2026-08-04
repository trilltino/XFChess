//! Client-side puzzle mode (docs/PUZZLES.md §10) — full play loop.
//!
//! The client is a renderer + input collector, never the judge. Flow:
//!   1. Menu sets [`PendingPuzzleRequest`] → `GET /puzzle/next`.
//!   2. On load: spawn the position (FEN-driven board), transition to `InGame`,
//!      and play the opponent's setup move so the player faces the tactic.
//!   3. Each player move → `POST /puzzle/move`; the server verifies one move at a
//!      time and returns the opponent's reply (revealed only after a correct
//!      move — future solution moves never reach the client).
//!   4. On the final correct move the server pays any bounty; we show the result.
//!
//! Networking is blocking (reqwest) so it runs on worker threads and reports
//! back over crossbeam channels polled each frame, like `WalletBridgePoller`.

use bevy::prelude::*;
use crossbeam_channel::{unbounded, Receiver};
use serde::Deserialize;

use crate::core::states::{GameMode, GameState};
use crate::engine::board_state::ChessEngine;
use crate::game::events::{MoveMadeEvent, NetworkMoveEvent};
use crate::multiplayer::network::vps::{client, vps_base};
use crate::rendering::pieces::{PieceType, PiecesSpawned};

/// Which puzzle experience the player picked from the menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PuzzleMode {
    Solve,
    Earn,
}

impl PuzzleMode {
    fn as_str(self) -> &'static str {
        match self {
            PuzzleMode::Solve => "solve",
            PuzzleMode::Earn => "earn",
        }
    }
}

/// Set by the menu when a puzzle button is clicked; consumed by `start_request`.
#[derive(Resource, Clone)]
pub struct PendingPuzzleRequest {
    pub mode: PuzzleMode,
    pub wallet: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PuzzleData {
    #[serde(default)]
    pub nonce: String,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub fen: String,
    /// The opponent's setup move (line[0]) — leaks nothing about the solution.
    #[serde(default)]
    pub setup_move: String,
    #[serde(default)]
    pub color: String,
    #[serde(default)]
    pub rating: i64,
    #[serde(default)]
    pub solution_len: i64,
    #[serde(default)]
    pub reward_lamports: Option<i64>,
    #[serde(default)]
    pub exhausted: bool,
    #[serde(default)]
    pub already_attempted: bool,
}

/// Backend `POST /puzzle/move` result.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct MoveOutcome {
    #[serde(default)]
    pub correct: bool,
    #[serde(default)]
    pub done: bool,
    #[serde(default)]
    pub win: bool,
    #[serde(default)]
    pub reply: Option<String>,
    #[serde(default)]
    pub rating: i64,
    #[serde(default)]
    pub rating_diff: i64,
    #[serde(default)]
    pub payout_sig: Option<String>,
    #[serde(default)]
    pub paid_lamports: i64,
}

enum NetMsg {
    Loaded(PuzzleData),
    Move(MoveOutcome),
    Error(String),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum PuzzlePhase {
    #[default]
    Idle,
    Loading,
    /// Transitioned to InGame; waiting for the board to spawn before the setup move.
    AwaitingBoard,
    /// Player to move.
    Playing,
    /// A move was submitted; waiting for the server's verdict/reply.
    AwaitingServer,
    /// Puzzle finished (solved or failed).
    Done,
}

/// Live puzzle state shared with the rest of the client (UI, input).
#[derive(Resource, Default)]
pub struct PuzzleSession {
    pub active: Option<PuzzleData>,
    pub phase: PuzzlePhase,
    pub moves: Vec<String>,
    pub status: String,
    pub last_result: Option<MoveOutcome>,
    rx: Option<Receiver<NetMsg>>,
}

/// Tells the board spawner to build a specific FEN instead of the start position.
#[derive(Resource, Default)]
pub struct PuzzleBoard {
    pub active: bool,
    pub fen: String,
}

pub struct PuzzlePlugin;

impl Plugin for PuzzlePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PuzzleSession>()
            .init_resource::<PuzzleBoard>()
            .add_systems(Update, (start_request, poll_net))
            .add_systems(
                Update,
                (puzzle_driver, capture_player_move).run_if(in_state(GameState::InGame)),
            )
            .add_systems(OnExit(GameState::InGame), clear_puzzle_board);
    }
}

// ── 1. Request a puzzle ──────────────────────────────────────────────────────

fn start_request(
    mut commands: Commands,
    req: Option<Res<PendingPuzzleRequest>>,
    mut session: ResMut<PuzzleSession>,
) {
    let Some(req) = req else { return };
    commands.remove_resource::<PendingPuzzleRequest>();
    if session.phase == PuzzlePhase::Loading {
        return;
    }
    // Earn mode pays out on completion, so it still requires a real wallet.
    // Solve mode has no reward and is open to Guest play — the identifier
    // just needs to be *something* (a node ID or local username both work)
    // so the server can track solve-attempt state for this session. See
    // docs/plans/identity-implementation-plan.md.
    if req.wallet.trim().is_empty() && req.mode == PuzzleMode::Earn {
        warn!("[puzzle] no wallet connected — cannot request an Earn puzzle");
        session.status = "connect a wallet first".into();
        return;
    }
    if req.wallet.trim().is_empty() {
        warn!("[puzzle] no identifier available — cannot request a puzzle");
        session.status = "unable to start — no local identity available".into();
        return;
    }

    let mode = req.mode;
    let wallet = req.wallet.clone();
    let (tx, rx) = unbounded();
    session.rx = Some(rx);
    session.phase = PuzzlePhase::Loading;
    session.status = "loading".into();
    let base = vps_base();
    std::thread::spawn(move || {
        let _ = tx.send(fetch_next(&base, mode, &wallet));
    });
}

fn fetch_next(base: &str, mode: PuzzleMode, wallet: &str) -> NetMsg {
    let cl = match client() {
        Ok(c) => c,
        Err(e) => return NetMsg::Error(e),
    };
    // Base58 pubkeys are URL-safe (alphanumeric, no +/=), so no encoding needed.
    let url = format!(
        "{base}/puzzle/next?mode={}&wallet={}",
        mode.as_str(),
        wallet
    );
    match cl.get(&url).send().and_then(|r| r.json::<PuzzleData>()) {
        Ok(d) => NetMsg::Loaded(d),
        Err(e) => NetMsg::Error(e.to_string()),
    }
}

// ── 2/4. Poll network results (load + per-move verdicts) ─────────────────────

fn poll_net(
    mut session: ResMut<PuzzleSession>,
    mut board: ResMut<PuzzleBoard>,
    mut next_state: ResMut<NextState<GameState>>,
    mut game_mode: ResMut<GameMode>,
    mut net_moves: MessageWriter<NetworkMoveEvent>,
) {
    let Some(rx) = session.rx.as_ref() else {
        return;
    };
    let msg = match rx.try_recv() {
        Ok(m) => m,
        Err(_) => return,
    };
    session.rx = None;

    match msg {
        NetMsg::Loaded(d) => {
            if d.exhausted {
                session.phase = PuzzlePhase::Idle;
                session.status = "no puzzles available".into();
                return;
            }
            if d.already_attempted {
                session.phase = PuzzlePhase::Idle;
                session.status = "already attempted".into();
                return;
            }
            info!(
                "[puzzle] loaded {} (player plays {}, {} moves, reward {:?})",
                d.id, d.color, d.solution_len, d.reward_lamports
            );
            // Spawn the position (before the setup move) and enter the game.
            board.active = true;
            board.fen = d.fen.clone();
            *game_mode = GameMode::MultiplayerLocal; // both sides local: no AI auto-plays
            session.moves.clear();
            session.last_result = None;
            session.active = Some(d);
            session.phase = PuzzlePhase::AwaitingBoard;
            session.status = "loading board".into();
            next_state.set(GameState::InGame);
        }
        NetMsg::Move(outcome) => {
            if !outcome.correct {
                session.phase = PuzzlePhase::Done;
                session.status = "failed".into();
                session.last_result = Some(outcome);
                return;
            }
            if outcome.done {
                session.phase = PuzzlePhase::Done;
                session.status = if outcome.win { "solved" } else { "failed" }.into();
                if outcome.payout_sig.is_some() {
                    info!("[puzzle] paid out: {:?}", outcome.payout_sig);
                }
                session.last_result = Some(outcome);
                return;
            }
            // Correct, not done: play the opponent's reply, then it's our move again.
            if let Some(reply) = outcome.reply.as_ref().and_then(|u| uci_to_event(u)) {
                net_moves.write(reply);
            }
            session.phase = PuzzlePhase::Playing;
            session.status = "your move".into();
        }
        NetMsg::Error(e) => {
            warn!("[puzzle] network error: {e}");
            session.phase = PuzzlePhase::Idle;
            session.status = format!("error: {e}");
        }
    }
}

// ── 3. In-game driver: set the engine + play the setup move once board is up ──

fn puzzle_driver(
    mut session: ResMut<PuzzleSession>,
    board: Res<PuzzleBoard>,
    pieces_spawned: Res<PiecesSpawned>,
    mut engine: ResMut<ChessEngine>,
    mut net_moves: MessageWriter<NetworkMoveEvent>,
) {
    if session.phase != PuzzlePhase::AwaitingBoard || !board.active {
        return;
    }
    if !pieces_spawned.spawned {
        return; // board still spawning
    }

    // Authoritatively set the engine to the puzzle position (the reset on
    // entering InGame put it at the start position) and rebuild the move cache
    // so the setup move passes legality.
    if let Err(e) = engine.set_from_fen(&board.fen) {
        warn!("[puzzle] bad FEN: {e}");
        session.phase = PuzzlePhase::Done;
        session.status = "bad puzzle".into();
        return;
    }
    engine.rebuild_legal_move_cache();

    // Play the opponent's setup move (line[0]) to reach the player's position.
    if let Some(setup) = session.active.as_ref().map(|p| p.setup_move.clone()) {
        if let Some(ev) = uci_to_event(&setup) {
            net_moves.write(ev);
        }
    }
    session.phase = PuzzlePhase::Playing;
    session.status = "your move".into();
    info!("[puzzle] board ready, setup move played — your move");
}

// ── Capture the player's move and submit it ──────────────────────────────────

fn capture_player_move(
    mut session: ResMut<PuzzleSession>,
    mut moves: MessageReader<MoveMadeEvent>,
) {
    // Only the player's own (non-remote) move while it's their turn.
    let mut chosen: Option<String> = None;
    for ev in moves.read() {
        if ev.remote {
            continue;
        }
        if session.phase == PuzzlePhase::Playing {
            chosen = Some(coords_to_uci(ev.from, ev.to, promo_char(ev.promotion)));
        }
    }
    let Some(uci) = chosen else { return };
    let Some(active) = session.active.clone() else {
        return;
    };

    session.moves.push(uci.clone());
    session.phase = PuzzlePhase::AwaitingServer;
    session.status = "checking…".into();

    let (tx, rx) = unbounded();
    session.rx = Some(rx);
    let base = vps_base();
    let nonce = active.nonce.clone();
    std::thread::spawn(move || {
        let _ = tx.send(post_move(&base, &nonce, &uci));
    });
}

fn post_move(base: &str, nonce: &str, uci: &str) -> NetMsg {
    let cl = match client() {
        Ok(c) => c,
        Err(e) => return NetMsg::Error(e),
    };
    let url = format!("{base}/puzzle/move");
    let body = serde_json::json!({ "nonce": nonce, "uci": uci });
    match cl
        .post(&url)
        .json(&body)
        .send()
        .and_then(|r| r.json::<MoveOutcome>())
    {
        Ok(o) => NetMsg::Move(o),
        Err(e) => NetMsg::Error(e.to_string()),
    }
}

fn clear_puzzle_board(mut board: ResMut<PuzzleBoard>, mut session: ResMut<PuzzleSession>) {
    board.active = false;
    if session.phase != PuzzlePhase::Idle {
        session.phase = PuzzlePhase::Idle;
    }
}

// ── UCI <-> board coordinate helpers ─────────────────────────────────────────

/// Parse a UCI move ("e2e4" / "e7e8q") into a `NetworkMoveEvent`.
fn uci_to_event(uci: &str) -> Option<NetworkMoveEvent> {
    let b = uci.trim().as_bytes();
    if b.len() < 4 {
        return None;
    }
    let file = |c: u8| (c as char).to_ascii_lowercase() as i32 - 'a' as i32;
    let rank = |c: u8| (c as char) as i32 - '1' as i32;
    let (ff, fr, tf, tr) = (file(b[0]), rank(b[1]), file(b[2]), rank(b[3]));
    if ![ff, fr, tf, tr].iter().all(|v| (0..8).contains(v)) {
        return None;
    }
    let promotion = b.get(4).map(|&c| (c as char).to_ascii_lowercase());
    Some(NetworkMoveEvent {
        from: (ff as u8, fr as u8),
        to: (tf as u8, tr as u8),
        promotion,
        expected_fen: None,
    })
}

fn coords_to_uci(from: (u8, u8), to: (u8, u8), promo: Option<char>) -> String {
    let sq = |c: (u8, u8)| format!("{}{}", (b'a' + c.0) as char, c.1 + 1);
    let mut s = format!("{}{}", sq(from), sq(to));
    if let Some(p) = promo {
        s.push(p);
    }
    s
}

fn promo_char(p: Option<PieceType>) -> Option<char> {
    match p {
        Some(PieceType::Queen) => Some('q'),
        Some(PieceType::Rook) => Some('r'),
        Some(PieceType::Bishop) => Some('b'),
        Some(PieceType::Knight) => Some('n'),
        _ => None,
    }
}
