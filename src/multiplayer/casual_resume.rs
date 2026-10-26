//! Rejoining a casual (off-chain) online game after the app was closed or crashed.
//!
//! A wagered game resumes from its on-chain account (`resume.rs`); a casual game
//! has none, but every move is on the backend's Braid log. When a casual game
//! starts a small record is saved next to the device identity. If the app comes
//! back with the record still there, the main menu offers to rejoin: the log is
//! replayed for legality, the session is restored, and the opponent is looked up
//! and bootstrapped on the gossip mesh again. The record is deleted when the game
//! is decided or the player dismisses it.
//!
//! The clock is not restored: it restarts at the game's full time control.

use bevy::prelude::*;
use braid_chess::ChessMessage;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::sync::oneshot;

use crate::multiplayer::session_restore::{
    opponent_node, replay_moves, restore_session, side_to_move_can_play, MoveReplay,
    RestoredSession, SessionTargets, SessionTransitions,
};
use crate::rendering::pieces::PieceColor;

/// A record older than this is ignored: the opponent has long since left.
const RECORD_MAX_AGE_MS: u64 = 24 * 60 * 60 * 1000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CasualGameRecord {
    /// The id the session and Braid log are keyed by.
    pub game_id: String,
    /// `"white"` or `"black"`.
    pub my_color: String,
    pub peer_node_id: Option<String>,
    /// `0` for an untimed game.
    pub base_time_seconds: u32,
    pub increment_seconds: u16,
    pub opponent_name: Option<String>,
    pub saved_at_ms: u64,
}

impl CasualGameRecord {
    fn color(&self) -> Option<PieceColor> {
        match self.my_color.as_str() {
            "white" => Some(PieceColor::White),
            "black" => Some(PieceColor::Black),
            _ => None,
        }
    }

    fn is_stale(&self, now_ms: u64) -> bool {
        now_ms.saturating_sub(self.saved_at_ms) > RECORD_MAX_AGE_MS
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as u64)
}

/// One record per installation; a second instance on the same machine
/// (`XFCHESS_NODE_KEY_PATH`) keeps its own, like its device id.
fn record_path() -> PathBuf {
    match std::env::var("XFCHESS_NODE_KEY_PATH") {
        Ok(p) if !p.trim().is_empty() => PathBuf::from(format!("{p}.active_casual_game.json")),
        _ => crate::multiplayer::network::device_id::data_dir().join("active_casual_game.json"),
    }
}

fn load_record() -> Option<CasualGameRecord> {
    let text = std::fs::read_to_string(record_path()).ok()?;
    let record: CasualGameRecord = serde_json::from_str(&text).ok()?;
    if record.color().is_none() || record.is_stale(now_ms()) {
        clear_record();
        return None;
    }
    Some(record)
}

fn save_record(record: &CasualGameRecord) {
    match serde_json::to_string(record) {
        Ok(text) => {
            if let Err(e) = std::fs::write(record_path(), text) {
                warn!("[CASUAL-RESUME] could not save the active game record: {e}");
            }
        }
        Err(e) => warn!("[CASUAL-RESUME] could not encode the active game record: {e}"),
    }
}

fn clear_record() {
    if let Err(e) = std::fs::remove_file(record_path()) {
        if e.kind() != std::io::ErrorKind::NotFound {
            warn!("[CASUAL-RESUME] could not remove the active game record: {e}");
        }
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum CasualResumeError {
    #[error("game already ended: {0}")]
    Ended(String),
    #[error("invalid move history: {0}")]
    InvalidLog(String),
    #[error("game data unavailable: {0}")]
    Unavailable(String),
}

impl CasualResumeError {
    /// Player-facing text; technical detail goes to the log.
    fn player_message(&self) -> String {
        match self {
            Self::Ended(how) => format!("That game already ended ({how})."),
            Self::InvalidLog(_) => {
                "The game's move history could not be verified, so it can't be rejoined.".into()
            }
            Self::Unavailable(_) => {
                "Couldn't reach the game right now. Check your connection and try again.".into()
            }
        }
    }

    /// Whether the saved record can never become useful again.
    fn is_final(&self) -> bool {
        matches!(self, Self::Ended(_))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CasualPlan {
    pub replay: MoveReplay,
    pub last_move_uci: Option<String>,
}

/// Decide whether a casual game can be rejoined from its event log.
pub fn plan_casual_resume(
    my_color: PieceColor,
    events: &[ChessMessage],
) -> Result<CasualPlan, CasualResumeError> {
    let mut moves = Vec::new();
    for event in events {
        match event {
            ChessMessage::Move(m) => moves.push(m),
            ChessMessage::Resign { player } => {
                return Err(CasualResumeError::Ended(format!("{player} resigned")))
            }
            ChessMessage::AcceptDraw { .. } => {
                return Err(CasualResumeError::Ended("draw agreed".into()))
            }
            _ => {}
        }
    }
    let replay = replay_moves(my_color, &moves, 0).map_err(CasualResumeError::InvalidLog)?;
    if !side_to_move_can_play(&replay.fen) {
        return Err(CasualResumeError::Ended("decided on the board".into()));
    }
    Ok(CasualPlan {
        replay,
        last_move_uci: moves.last().map(|m| m.uci.clone()),
    })
}

struct CasualInputs {
    plan: CasualPlan,
    peer_node_id: Option<String>,
}

fn gather(
    record: CasualGameRecord,
    local_node_id: String,
) -> Result<CasualInputs, CasualResumeError> {
    let my_color = record
        .color()
        .ok_or_else(|| CasualResumeError::InvalidLog("saved colour is unreadable".into()))?;
    let game_id = record.game_id.as_str();

    let mut events = crate::multiplayer::vps_client::fetch_game_events_by_id(game_id)
        .map_err(CasualResumeError::Unavailable)?;
    // Moves this device made that never reached the backend are still in its journal.
    let journal = crate::multiplayer::network::braid_transport::open_journal(
        &crate::multiplayer::vps_client::vps_base(),
        game_id,
    )
    .map_err(|e| CasualResumeError::Unavailable(format!("pending game events: {e}")))?;
    let pending = journal
        .pending()
        .map_err(|e| CasualResumeError::Unavailable(format!("pending game events: {e}")))?;
    let mut known: std::collections::HashSet<String> = events
        .iter()
        .filter_map(|event| match event {
            ChessMessage::Move(payload) => Some(braid_chess::version_hash(
                &payload.fen_after,
                payload.move_number,
            )),
            _ => None,
        })
        .collect();
    for write in pending {
        if write.sender == local_node_id
            && matches!(write.message, ChessMessage::Move(_))
            && known.insert(write.version)
        {
            events.push(write.message);
        }
    }

    let plan = plan_casual_resume(my_color, &events)?;
    // The backend's record of who played is authoritative; fall back to what this
    // device saw when the game began (direct connections register nothing).
    let peer_node_id = crate::multiplayer::vps_client::fetch_game_nodes(game_id)
        .unwrap_or_else(|e| {
            warn!("[CASUAL-RESUME] could not look up the opponent node: {e}");
            None
        })
        .and_then(|nodes| opponent_node(&nodes, &local_node_id))
        .or(record.peer_node_id);
    Ok(CasualInputs { plan, peer_node_id })
}

#[derive(Debug, Clone, Default, PartialEq)]
pub enum CasualResumeStatus {
    #[default]
    Idle,
    Checking,
    Failed(String),
}

#[derive(Resource, Default)]
pub struct CasualResumeState {
    /// The game that can be rejoined, if one was left running.
    pub record: Option<CasualGameRecord>,
    pub status: CasualResumeStatus,
    rx: Option<oneshot::Receiver<Result<CasualInputs, CasualResumeError>>>,
}

impl CasualResumeState {
    fn request(&mut self, local_node_id: String) {
        let Some(record) = self.record.clone() else {
            return;
        };
        if self.rx.is_some() {
            return;
        }
        let (tx, rx) = oneshot::channel();
        crate::core::net_task::spawn_detached("casual-resume", move || {
            let _ = tx.send(gather(record, local_node_id));
        });
        self.rx = Some(rx);
        self.status = CasualResumeStatus::Checking;
    }

    fn dismiss(&mut self) {
        clear_record();
        self.record = None;
        self.status = CasualResumeStatus::Idle;
    }
}

fn load_saved_record(mut state: ResMut<CasualResumeState>) {
    state.record = load_record();
    if let Some(record) = &state.record {
        info!(
            "[CASUAL-RESUME] game {} was left running; offering to rejoin",
            record.game_id
        );
    }
}

/// Remember a casual game as it starts so it can be rejoined after a restart.
fn record_casual_game_start(
    mut started: MessageReader<crate::game::events::GameStartedEvent>,
    mut state: ResMut<CasualResumeState>,
    session: Res<crate::multiplayer::network::online_game_session::OnlineGameSession>,
    p2p: Res<crate::multiplayer::network::p2p::P2PConnectionState>,
    time_control: Res<crate::game::resources::active_time_control::ActiveTimeControl>,
    mode: Res<crate::core::GameMode>,
    #[cfg(feature = "solana")] sync: Option<Res<crate::multiplayer::solana::addon::SolanaGameSync>>,
) {
    if started.read().count() == 0 {
        return;
    }
    #[cfg(feature = "solana")]
    let on_chain = sync.is_some_and(|s| s.game_id.is_some());
    #[cfg(not(feature = "solana"))]
    let on_chain = false;
    // Wagered and tournament games resume from the chain, not from here.
    if *mode != crate::core::GameMode::OnlineMultiplayer || on_chain || !session.is_configured() {
        return;
    }
    let Some(color) = p2p.player_color else {
        return;
    };
    if state
        .record
        .as_ref()
        .is_some_and(|known| known.game_id == session.game_id)
    {
        return;
    }
    let control = time_control.control;
    let record = CasualGameRecord {
        game_id: session.game_id.clone(),
        my_color: match color {
            PieceColor::White => "white".into(),
            PieceColor::Black => "black".into(),
        },
        peer_node_id: p2p.peer_node_id.clone(),
        base_time_seconds: control.base_seconds(),
        increment_seconds: control.increment_seconds(),
        opponent_name: p2p.opponent_display_name.clone(),
        saved_at_ms: now_ms(),
    };
    save_record(&record);
    state.record = Some(record);
}

/// A game that is decided has nothing left to rejoin.
fn forget_decided_game(
    mut state: ResMut<CasualResumeState>,
    game_over: Res<crate::game::resources::GameOverState>,
) {
    if state.record.is_some() && game_over.is_game_over() {
        state.dismiss();
    }
}

fn poll_casual_resume(
    mut state: ResMut<CasualResumeState>,
    mut t: SessionTargets,
    mut go: SessionTransitions,
    #[cfg(feature = "solana")] mut solana_sync: Option<
        ResMut<crate::multiplayer::solana::addon::SolanaGameSync>,
    >,
    #[cfg(feature = "solana")] mut competitive: Option<
        ResMut<crate::multiplayer::solana::addon::CompetitiveMatchState>,
    >,
) {
    let Some(rx) = state.rx.as_mut() else {
        return;
    };
    let result = match rx.try_recv() {
        Ok(result) => result,
        Err(oneshot::error::TryRecvError::Empty) => return,
        Err(oneshot::error::TryRecvError::Closed) => Err(CasualResumeError::Unavailable(
            "rejoin check stopped unexpectedly".into(),
        )),
    };
    state.rx = None;
    let Some(record) = state.record.clone() else {
        return;
    };
    match result {
        Ok(inputs) => {
            let Some(my_color) = record.color() else {
                return;
            };
            // A wagered lobby or tournament left on-chain context behind; this game has none.
            #[cfg(feature = "solana")]
            crate::multiplayer::solana::addon::clear_on_chain_game_state(
                solana_sync.as_deref_mut(),
                competitive.as_deref_mut(),
            );
            let control = if record.base_time_seconds == 0 {
                crate::game::time_control::TimeControl::Unlimited
            } else {
                crate::game::time_control::TimeControl::Custom {
                    base_seconds: record.base_time_seconds,
                    increment_seconds: record.increment_seconds,
                }
            };
            let game_id =
                crate::multiplayer::network::online_game_session::numeric_game_id(&record.game_id);
            restore_session(
                &RestoredSession {
                    session_game_id: record.game_id.clone(),
                    game_id,
                    my_color,
                    replay: &inputs.plan.replay,
                    last_move_uci: inputs.plan.last_move_uci.clone(),
                    wager_sol: 0.0,
                    base_time_seconds: u64::from(record.base_time_seconds),
                    increment_seconds: record.increment_seconds,
                    time_control: Some(control),
                    peer_node_id: inputs.peer_node_id,
                },
                &mut t,
                &mut go,
            );
            t.set_opponent_name(record.opponent_name.clone());
            state.status = CasualResumeStatus::Idle;
            info!(
                "[CASUAL-RESUME] game {} rejoined as {:?} at ply {}",
                record.game_id,
                my_color,
                inputs.plan.replay.my_moves + inputs.plan.replay.opponent_moves
            );
        }
        Err(e) => {
            warn!(
                "[CASUAL-RESUME] game {} not rejoined: {e:?}",
                record.game_id
            );
            let message = e.player_message();
            if e.is_final() {
                state.dismiss();
            }
            state.status = CasualResumeStatus::Failed(message);
        }
    }
}

/// The main-menu offer to rejoin a game that was left running.
fn casual_resume_prompt(
    mut contexts: bevy_egui::EguiContexts,
    mut state: ResMut<CasualResumeState>,
    network_state: Option<Res<crate::multiplayer::OnlineNetworkState>>,
) {
    use bevy_egui::egui;

    // A failure for a game that has since been forgotten is still worth one line.
    let failure = match &state.status {
        CasualResumeStatus::Failed(message) => Some(message.clone()),
        _ => None,
    };
    if state.record.is_none() && failure.is_none() {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else { return };
    let local_node = network_state
        .as_deref()
        .and_then(|n| n.node_id.as_ref())
        .map(|id| bs58::encode(id.as_bytes()).into_string());
    let checking = state.status == CasualResumeStatus::Checking;

    let mut rejoin = false;
    let mut dismiss = false;
    egui::Window::new("casual_resume_prompt")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_TOP, [0.0, 16.0])
        .show(ctx, |ui| {
            if let Some(record) = &state.record {
                let opponent = record.opponent_name.as_deref().unwrap_or("your opponent");
                ui.label(
                    egui::RichText::new(format!("A game against {opponent} is still in progress."))
                        .strong(),
                );
            }
            if let Some(message) = &failure {
                ui.colored_label(egui::Color32::from_rgb(255, 160, 120), message);
            }
            ui.horizontal(|ui| {
                if state.record.is_some() {
                    if checking {
                        ui.spinner();
                        ui.label("Verifying game…");
                    } else if ui
                        .add_enabled(local_node.is_some(), egui::Button::new("Rejoin game"))
                        .clicked()
                    {
                        rejoin = true;
                    }
                }
                if ui.button("Dismiss").clicked() {
                    dismiss = true;
                }
            });
        });

    if rejoin {
        if let Some(node) = local_node {
            state.request(node);
        }
    }
    if dismiss {
        state.dismiss();
    }
}

pub struct CasualResumePlugin;

impl Plugin for CasualResumePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CasualResumeState>()
            .add_systems(Startup, load_saved_record)
            .add_systems(
                Update,
                (record_casual_game_start, poll_casual_resume)
                    .chain()
                    .in_set(crate::core::schedule::SessionSet::Match),
            )
            .add_systems(
                Update,
                forget_decided_game.run_if(in_state(crate::core::GameState::InGame)),
            )
            .add_systems(
                bevy_egui::EguiPrimaryContextPass,
                casual_resume_prompt.run_if(in_state(crate::core::MenuState::Main)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use braid_chess::MovePayload;

    const E4: &str = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1";
    const E4E5: &str = "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e6 0 2";

    fn mv(uci: &str, fen: &str, n: u32) -> ChessMessage {
        ChessMessage::Move(MovePayload::from_uci(uci, fen, n, "node"))
    }

    fn log() -> Vec<ChessMessage> {
        vec![mv("e2e4", E4, 1), mv("e7e5", E4E5, 1)]
    }

    #[test]
    fn a_running_game_can_be_rejoined_from_its_log() -> Result<(), CasualResumeError> {
        let plan = plan_casual_resume(PieceColor::Black, &log())?;
        assert_eq!(plan.replay.fen, E4E5);
        assert_eq!(
            (plan.replay.my_moves, plan.replay.opponent_moves),
            (1, 1),
            "black played e7e5, white e2e4"
        );
        assert_eq!(plan.last_move_uci.as_deref(), Some("e7e5"));
        Ok(())
    }

    #[test]
    fn a_game_with_no_moves_yet_is_rejoinable() -> Result<(), CasualResumeError> {
        let plan = plan_casual_resume(PieceColor::White, &[])?;
        assert_eq!(plan.replay.my_moves + plan.replay.opponent_moves, 0);
        assert_eq!(plan.last_move_uci, None);
        Ok(())
    }

    #[test]
    fn a_game_the_log_shows_as_over_is_not_rejoined() {
        let mut resigned = log();
        resigned.push(ChessMessage::Resign {
            player: "white".into(),
        });
        assert!(matches!(
            plan_casual_resume(PieceColor::Black, &resigned),
            Err(CasualResumeError::Ended(_))
        ));

        let mut drawn = log();
        drawn.push(ChessMessage::AcceptDraw {
            player: "white".into(),
            ply: 2,
            at_ms: 1,
        });
        assert!(matches!(
            plan_casual_resume(PieceColor::Black, &drawn),
            Err(CasualResumeError::Ended(_))
        ));
    }

    #[test]
    fn a_log_that_ends_in_mate_is_not_rejoined() {
        let fools_mate = vec![
            mv(
                "f2f3",
                "rnbqkbnr/pppppppp/8/8/8/5P2/PPPPP1PP/RNBQKBNR b KQkq - 0 1",
                1,
            ),
            mv(
                "e7e5",
                "rnbqkbnr/pppp1ppp/8/4p3/8/5P2/PPPPP1PP/RNBQKBNR w KQkq e6 0 2",
                1,
            ),
            mv(
                "g2g4",
                "rnbqkbnr/pppp1ppp/8/4p3/6P1/5P2/PPPPP2P/RNBQKBNR b KQkq g3 0 2",
                2,
            ),
            mv(
                "d8h4",
                "rnb1kbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 1 3",
                2,
            ),
        ];
        assert!(matches!(
            plan_casual_resume(PieceColor::White, &fools_mate),
            Err(CasualResumeError::Ended(_))
        ));
    }

    #[test]
    fn a_tampered_log_is_refused() {
        let bad = vec![mv("e2e5", E4, 1)];
        assert!(matches!(
            plan_casual_resume(PieceColor::White, &bad),
            Err(CasualResumeError::InvalidLog(_))
        ));
    }

    #[test]
    fn only_a_finished_game_makes_the_record_worthless() {
        assert!(CasualResumeError::Ended("x".into()).is_final());
        assert!(!CasualResumeError::Unavailable("x".into()).is_final());
        assert!(!CasualResumeError::InvalidLog("x".into()).is_final());
    }

    #[test]
    fn records_expire_after_a_day() {
        let record = CasualGameRecord {
            game_id: "g".into(),
            my_color: "white".into(),
            peer_node_id: None,
            base_time_seconds: 300,
            increment_seconds: 0,
            opponent_name: None,
            saved_at_ms: 1_000,
        };
        assert!(!record.is_stale(1_000 + RECORD_MAX_AGE_MS));
        assert!(record.is_stale(1_001 + RECORD_MAX_AGE_MS));
        assert_eq!(record.color(), Some(PieceColor::White));
    }

    #[test]
    fn a_record_survives_a_round_trip_through_json() -> Result<(), serde_json::Error> {
        let record = CasualGameRecord {
            game_id: "lobby-9f".into(),
            my_color: "black".into(),
            peer_node_id: Some("peer".into()),
            base_time_seconds: 0,
            increment_seconds: 0,
            opponent_name: Some("Tino".into()),
            saved_at_ms: 5,
        };
        let decoded: CasualGameRecord = serde_json::from_str(&serde_json::to_string(&record)?)?;
        assert_eq!(decoded, record);
        Ok(())
    }
}
