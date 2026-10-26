//! Rebuilding a running online game from its Braid log.
//!
//! Shared by resuming a wagered game (verified against the chain, `resume.rs`) and
//! rejoining a casual one (`casual_resume.rs`): both replay the logged moves for
//! legality, then restore transport sequencing, deduplication and the board.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use braid_chess::MovePayload;

use crate::engine::board_state::ChessEngine;
use crate::multiplayer::systems::fen_ply;
use crate::rendering::pieces::PieceColor;

pub const START_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

fn mover_of(fen_before: &str) -> PieceColor {
    if fen_before.split_whitespace().nth(1) == Some("b") {
        PieceColor::Black
    } else {
        PieceColor::White
    }
}

/// The position and sequencing state a legal move log leads to.
#[derive(Debug, Clone, PartialEq)]
pub struct MoveReplay {
    /// Position after the last logged move.
    pub fen: String,
    pub my_moves: u32,
    pub opponent_moves: u32,
    /// Version of this player's last move (their Braid/gossip lane head).
    pub my_head: String,
    /// Version of the opponent's last move (seeds their causal lane).
    pub opponent_head: String,
    pub applied_versions: Vec<String>,
    /// Position after the first `checkpoint` moves (what the chain has recorded).
    pub checkpoint_fen: String,
}

/// Replay `moves` from the start position, refusing the first illegal move or one
/// that reports the wrong ply. `checkpoint` picks the move count whose position is
/// reported separately.
pub fn replay_moves(
    my_color: PieceColor,
    moves: &[&MovePayload],
    checkpoint: usize,
) -> Result<MoveReplay, String> {
    let mut engine = ChessEngine::default();
    let mut prev = START_FEN.to_string();
    let mut my_moves = 0u32;
    let mut opponent_moves = 0u32;
    let mut my_head = "0".to_string();
    let mut opponent_head = "0".to_string();
    let mut applied_versions = Vec::with_capacity(moves.len());
    let mut checkpoint_fen = START_FEN.to_string();
    for (i, m) in moves.iter().enumerate() {
        engine
            .set_from_fen(&prev)
            .map_err(|e| format!("position before move {}: {e}", i + 1))?;
        engine.rebuild_legal_move_cache();
        if !engine.is_move_legal_by_uci(&m.uci) {
            return Err(format!("move {} ({}) is illegal", i + 1, m.uci));
        }
        if fen_ply(&m.fen_after) != Some(i as u64 + 1) {
            return Err(format!(
                "move {} reports the wrong ply ({})",
                i + 1,
                m.fen_after
            ));
        }
        let version = braid_chess::version_hash(&m.fen_after, m.move_number);
        if mover_of(&prev) == my_color {
            my_moves += 1;
            my_head = version.clone();
        } else {
            opponent_moves += 1;
            opponent_head = version.clone();
        }
        applied_versions.push(version);
        prev = m.fen_after.clone();
        if i + 1 == checkpoint {
            checkpoint_fen = prev.clone();
        }
    }
    Ok(MoveReplay {
        fen: prev,
        my_moves,
        opponent_moves,
        my_head,
        opponent_head,
        applied_versions,
        checkpoint_fen,
    })
}

/// Whether the side to move in `fen` has any legal move; `false` means the game
/// was already decided on the board (mate or stalemate).
pub fn side_to_move_can_play(fen: &str) -> bool {
    let mut engine = ChessEngine::default();
    if engine.set_from_fen(fen).is_err() {
        return true;
    }
    engine.rebuild_legal_move_cache();
    engine.has_legal_moves()
}

/// Everything a restored session needs beyond the replay itself.
pub struct RestoredSession<'a> {
    /// The id the session and the Braid log are keyed by (numeric for wagered games,
    /// possibly any string for lobby games).
    pub session_game_id: String,
    /// [`numeric_game_id`](crate::multiplayer::network::online_game_session::numeric_game_id)
    /// of `session_game_id`.
    pub game_id: u64,
    pub my_color: PieceColor,
    pub replay: &'a MoveReplay,
    pub last_move_uci: Option<String>,
    pub wager_sol: f64,
    pub base_time_seconds: u64,
    pub increment_seconds: u16,
    /// Overrides the clock derived from `base_time_seconds`, which cannot express
    /// an untimed game.
    pub time_control: Option<crate::game::time_control::TimeControl>,
    /// The opponent's transport node id, when it could be learned.
    pub peer_node_id: Option<String>,
}

/// Position to apply to the engine once the restored board has spawned.
#[derive(Resource, Default)]
pub struct PendingResumeFen(pub Option<String>);

#[derive(SystemParam)]
pub struct SessionTargets<'w> {
    online_session: ResMut<'w, crate::multiplayer::network::online_game_session::OnlineGameSession>,
    network_config: Res<'w, crate::multiplayer::types::NetworkConfig>,
    network_state: Option<Res<'w, crate::multiplayer::OnlineNetworkState>>,
    p2p: ResMut<'w, crate::multiplayer::network::p2p::P2PConnectionState>,
    causal: ResMut<'w, crate::multiplayer::types::CausalChainState>,
    pending: ResMut<'w, crate::multiplayer::types::PendingMoveBuffer>,
    barrier: ResMut<'w, crate::multiplayer::types::OnlineStartBarrier>,
    resume_board: ResMut<'w, crate::rendering::pieces::ResumeBoard>,
    time_control: ResMut<'w, crate::game::resources::active_time_control::ActiveTimeControl>,
    pending_fen: ResMut<'w, PendingResumeFen>,
}

#[derive(SystemParam)]
pub struct SessionTransitions<'w> {
    ai_config: ResMut<'w, crate::game::ai::resource::ChessAIResource>,
    core_mode: ResMut<'w, crate::core::GameMode>,
    next_state: ResMut<'w, NextState<crate::core::GameState>>,
    menu_state: ResMut<'w, NextState<crate::core::MenuState>>,
    started: MessageWriter<'w, crate::game::events::GameStartedEvent>,
}

/// The other player's node id, given the game's (host, joiner) pair and ours.
pub fn opponent_node(nodes: &(String, String), local: &str) -> Option<String> {
    if nodes.0 == local {
        Some(nodes.1.clone())
    } else if nodes.1 == local {
        Some(nodes.0.clone())
    } else {
        None
    }
}

impl SessionTargets<'_> {
    /// Show the opponent's name again after the restart that forgot it.
    pub fn set_opponent_name(&mut self, name: Option<String>) {
        if name.is_some() {
            self.p2p.opponent_display_name = name;
        }
    }

    /// This installation's transport node id, base58.
    pub fn local_node_id(&self) -> Option<String> {
        self.network_state
            .as_deref()?
            .node_id
            .as_ref()
            .map(|id| bs58::encode(id.as_bytes()).into_string())
    }
}

/// Put the session, transport and board state back to where the log ends, and
/// enter the game.
pub fn restore_session(
    plan: &RestoredSession,
    t: &mut SessionTargets,
    go: &mut SessionTransitions,
) {
    use crate::multiplayer::network::online_game_session::{numeric_game_id, start_session};

    let game_id = plan.game_id;
    let replay = plan.replay;
    if let Some(network_state) = t.network_state.as_deref() {
        start_session(
            &mut t.online_session,
            t.network_config.vps_base_url.clone(),
            plan.session_game_id.clone(),
            plan.wager_sol,
            network_state,
        );
    }
    // Continue this player's own Braid/gossip lane where the log ends.
    t.online_session.next_move_number = replay.my_moves + 1;
    t.online_session.next_nonce = replay.my_moves as u64 + 1;
    t.online_session.last_published_move_version = replay.my_head.clone();
    t.online_session.last_move_uci = plan.last_move_uci.clone();

    let net_game_id = numeric_game_id(&plan.session_game_id);
    // Log replay on resubscribe must not re-apply moves the snapshot already has.
    t.causal
        .applied_versions
        .entry(net_game_id)
        .or_default()
        .extend(replay.applied_versions.iter().cloned());
    t.causal.resume_seeds.insert(
        net_game_id,
        (replay.opponent_moves as u64, replay.opponent_head.clone()),
    );
    t.pending.sequencers.insert(
        net_game_id,
        crate::multiplayer::network::reorder::NonceSequencer::starting_at(
            replay.opponent_moves as u64 + 1,
            crate::multiplayer::types::PendingMoveBuffer::MAX_BUFFERED,
        ),
    );
    // Both players were present when the game began.
    *t.barrier = crate::multiplayer::types::OnlineStartBarrier {
        game_id: net_game_id,
        local_ready: true,
        remote_ready: true,
        start_confirmed: true,
        ready_sent: true,
        start_sent: true,
        remote_started: true,
    };

    t.p2p.is_host = plan.my_color == PieceColor::White;
    t.p2p.player_color = Some(plan.my_color);
    t.p2p.game_id = Some(game_id);
    t.p2p.status = crate::multiplayer::network::p2p::P2PConnectionStatus::InGame;
    if let Some(network_state) = t.network_state.as_deref() {
        t.p2p.local_node_id = network_state.node_id;
        // Knowing the opponent lets the liveness check watch for them and lets the
        // gossip mesh find them again after a restart.
        if let Some(peer) = plan.peer_node_id.as_deref() {
            t.p2p.peer_node_id = Some(peer.to_string());
            crate::multiplayer::network::p2p::bootstrap_known_peer(network_state, peer);
        }
    }

    t.resume_board.fen = Some(replay.fen.clone());
    t.pending_fen.0 = Some(replay.fen.clone());
    if let Some(control) = plan.time_control {
        t.time_control.control = control;
    } else if plan.base_time_seconds > 0 {
        t.time_control.control = crate::game::time_control::TimeControl::Custom {
            base_seconds: plan.base_time_seconds.min(u32::MAX as u64) as u32,
            increment_seconds: plan.increment_seconds,
        };
    }
    crate::multiplayer::network::game_id_store::set(game_id);

    go.ai_config.mode = crate::game::ai::resource::AiMode::Multiplayer;
    *go.core_mode = crate::core::GameMode::OnlineMultiplayer;
    go.started
        .write(crate::game::events::GameStartedEvent { game_id });
    go.next_state.set(crate::core::GameState::InGame);
    go.menu_state.set(crate::core::MenuState::Main);
}

/// Once the restored position's pieces exist, put the engine and turn state on
/// the same position (entering `InGame` reset them to the start position).
fn apply_resume_board(
    mut pending_fen: ResMut<PendingResumeFen>,
    pieces_spawned: Res<crate::rendering::pieces::PiecesSpawned>,
    mut resume_board: ResMut<crate::rendering::pieces::ResumeBoard>,
    mut engine: ResMut<ChessEngine>,
    mut current_turn: ResMut<crate::game::resources::CurrentTurn>,
    mut turn_context: ResMut<crate::game::resources::TurnStateContext>,
) {
    if !pieces_spawned.spawned {
        return;
    }
    let Some(fen) = pending_fen.0.take() else {
        return;
    };
    if let Err(e) = engine.set_from_fen(&fen) {
        error!("[RESUME] could not load resumed position {fen}: {e}");
        return;
    }
    engine.rebuild_legal_move_cache();
    let side = engine.side_to_move();
    let fullmove = fen
        .split_whitespace()
        .nth(5)
        .and_then(|n| n.parse::<u32>().ok())
        .unwrap_or(1);
    current_turn.color = side;
    current_turn.move_number = fullmove;
    turn_context.current_player = side;
    turn_context.move_number = fullmove;
    resume_board.fen = None;
    info!("[RESUME] Board restored to {fen}");
}

fn clear_restore_on_exit(
    mut pending_fen: ResMut<PendingResumeFen>,
    mut resume_board: ResMut<crate::rendering::pieces::ResumeBoard>,
) {
    pending_fen.0 = None;
    resume_board.fen = None;
}

pub struct SessionRestorePlugin;

impl Plugin for SessionRestorePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PendingResumeFen>()
            .add_systems(
                Update,
                apply_resume_board.in_set(crate::core::schedule::MoveSource::Resume),
            )
            .add_systems(
                OnExit(crate::core::GameState::InGame),
                clear_restore_on_exit,
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const E4: &str = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1";
    const E4E5: &str = "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e6 0 2";

    fn mv(uci: &str, fen: &str, n: u32) -> MovePayload {
        MovePayload::from_uci(uci, fen, n, "node")
    }

    #[test]
    fn a_legal_log_replays_to_its_last_position_and_counts_each_side() -> Result<(), String> {
        let (a, b) = (mv("e2e4", E4, 1), mv("e7e5", E4E5, 1));
        let replay = replay_moves(PieceColor::White, &[&a, &b], 1)?;
        assert_eq!(replay.fen, E4E5);
        assert_eq!((replay.my_moves, replay.opponent_moves), (1, 1));
        assert_eq!(replay.checkpoint_fen, E4);
        assert_eq!(replay.applied_versions.len(), 2);
        assert_ne!(replay.my_head, replay.opponent_head);
        Ok(())
    }

    #[test]
    fn the_opponent_is_whichever_node_is_not_ours() {
        let nodes = ("host".to_string(), "joiner".to_string());
        assert_eq!(opponent_node(&nodes, "host").as_deref(), Some("joiner"));
        assert_eq!(opponent_node(&nodes, "joiner").as_deref(), Some("host"));
        assert_eq!(opponent_node(&nodes, "stranger"), None);
    }

    #[test]
    fn an_illegal_move_is_refused() {
        let bad = mv("e2e5", E4, 1);
        assert!(replay_moves(PieceColor::White, &[&bad], 0).is_err());
    }

    #[test]
    fn a_move_reporting_the_wrong_ply_is_refused() {
        let skipped = mv("e2e4", E4E5, 1);
        assert!(replay_moves(PieceColor::White, &[&skipped], 0).is_err());
    }

    #[test]
    fn a_decided_board_has_no_side_able_to_play() {
        // Fool's mate, white to move and mated.
        assert!(!side_to_move_can_play(
            "rnb1kbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 1 3"
        ));
        assert!(side_to_move_can_play(START_FEN));
    }
}
