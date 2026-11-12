//! Resume from the on-chain Game PDA and the backend move log, replaying every
//! move for legality. Requeue logged moves ahead of the chain; reject a log behind
//! the chain, illegal moves, or an ended game. Restore transport sequencing and
//! deduplication state along with the board.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use braid_chess::{ChessMessage, MovePayload};
use solana_sdk::pubkey::Pubkey;
use tokio::sync::oneshot;
use xfchess_game::state::{Game, GameStatus};

use crate::multiplayer::session_restore::{
    opponent_node, replay_moves, restore_session, MoveReplay, RestoredSession, SessionTargets,
    SessionTransitions,
};
use crate::rendering::pieces::PieceColor;

pub use crate::multiplayer::session_restore::START_FEN;

/// Moves the log may be ahead of the chain (unrecorded batch). Beyond this the
/// gap is not a normal batching lag and is refused for investigation.
pub const MAX_UNRECORDED_MOVES: usize = 40;

#[derive(Debug, Clone, PartialEq)]
pub struct ResumePlan {
    pub game_id: u64,
    pub my_color: PieceColor,
    /// Current position (after the last logged move).
    pub fen: String,
    pub my_moves: u32,
    pub opponent_moves: u32,
    /// Version of this player's last move (their Braid/gossip lane head).
    pub my_head: String,
    /// Version of the opponent's last move (seeds their causal lane).
    pub opponent_head: String,
    pub applied_versions: Vec<String>,
    /// Position after the moves the chain has recorded (rollup baseline).
    pub chain_fen: String,
    pub chain_move_count: u16,
    /// Logged moves the chain has not recorded yet: (uci, fen_after).
    pub unrecorded: Vec<(String, String)>,
    /// The last logged move; the next move's signed authorisation answers it.
    pub last_move_uci: Option<String>,
    pub wager_lamports: u64,
    pub base_time_seconds: u64,
    pub increment_seconds: u16,
    pub is_delegated: bool,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ResumeError {
    #[error("wallet is not a game participant")]
    NotParticipant,
    #[error("game is not in progress: {0}")]
    NotInProgress(String),
    #[error("game already ended: {0}")]
    Ended(String),
    #[error("move log has {log} moves but chain has {chain}")]
    LogBehindChain { log: usize, chain: u16 },
    #[error("move log is too far ahead: log={log}, chain={chain}")]
    LogTooFarAhead { log: usize, chain: u16 },
    #[error("invalid move history: {0}")]
    InvalidLog(String),
    #[error("game data unavailable: {0}")]
    Unavailable(String),
}

impl ResumeError {
    /// Player-facing text; technical detail goes to the log.
    pub fn player_message(&self) -> String {
        match self {
            Self::NotParticipant => "This wallet is not a player in that game.".into(),
            Self::NotInProgress(s) => format!("The game is not in progress ({s})."),
            Self::Ended(how) => {
                format!("The game already ended ({how}). It will settle automatically.")
            }
            Self::LogBehindChain { .. } | Self::LogTooFarAhead { .. } | Self::InvalidLog(_) => {
                "The game's move history could not be verified, so it can't be resumed safely. \
                 Your stake stays in escrow; contact support with the game id."
                    .into()
            }
            Self::Unavailable(_) => {
                "Couldn't reach the game right now. Check your connection and try again.".into()
            }
        }
    }
}

/// Plan a resume. `chain_move_count`/`game` come from the authoritative Game
/// account (the ER copy while delegated); `events` is the backend log.
pub fn plan_resume(
    game: &Game,
    chain_move_count: u16,
    is_delegated: bool,
    wallet: &Pubkey,
    events: &[ChessMessage],
) -> Result<ResumePlan, ResumeError> {
    let my_color = if game.white == *wallet {
        PieceColor::White
    } else if game.black == *wallet && game.black != Pubkey::default() {
        PieceColor::Black
    } else {
        return Err(ResumeError::NotParticipant);
    };
    match game.status {
        GameStatus::Active => {}
        GameStatus::Finished | GameStatus::Settled => {
            return Err(ResumeError::Ended(
                format!("{:?}", game.status).to_lowercase(),
            ))
        }
        other => {
            return Err(ResumeError::NotInProgress(
                format!("{other:?}").to_lowercase(),
            ))
        }
    }

    let mut moves: Vec<&MovePayload> = Vec::new();
    for event in events {
        match event {
            ChessMessage::Move(m) => moves.push(m),
            ChessMessage::Resign { player } => {
                return Err(ResumeError::Ended(format!("{player} resigned")))
            }
            ChessMessage::AcceptDraw { .. } => {
                return Err(ResumeError::Ended("draw agreed".into()))
            }
            _ => {}
        }
    }

    let chain = chain_move_count as usize;
    if moves.len() < chain {
        return Err(ResumeError::LogBehindChain {
            log: moves.len(),
            chain: chain_move_count,
        });
    }
    if moves.len() - chain > MAX_UNRECORDED_MOVES {
        return Err(ResumeError::LogTooFarAhead {
            log: moves.len(),
            chain: chain_move_count,
        });
    }

    let replay = replay_moves(my_color, &moves, chain).map_err(ResumeError::InvalidLog)?;

    Ok(ResumePlan {
        game_id: game.game_id,
        my_color,
        fen: replay.fen,
        my_moves: replay.my_moves,
        opponent_moves: replay.opponent_moves,
        my_head: replay.my_head,
        opponent_head: replay.opponent_head,
        applied_versions: replay.applied_versions,
        chain_fen: replay.checkpoint_fen,
        chain_move_count,
        unrecorded: moves[chain..]
            .iter()
            .map(|m| (m.uci.clone(), m.fen_after.clone()))
            .collect(),
        last_move_uci: moves.last().map(|m| m.uci.clone()),
        wager_lamports: game.wager_amount,
        base_time_seconds: game.base_time_seconds,
        increment_seconds: game.increment_seconds,
        is_delegated,
    })
}

struct ResumeInputs {
    plan: ResumePlan,
    /// Next `record_move` nonce from the chain (delegated games only).
    next_nonce: Option<u64>,
    /// Host and joiner transport node ids, when the backend recorded them.
    nodes: Option<(String, String)>,
}

fn gather(game_id: u64, wallet: Pubkey, rpc_url: String) -> Result<ResumeInputs, ResumeError> {
    use solana_client::rpc_client::RpcClient;
    use solana_commitment_config::CommitmentConfig;

    let program_id: Pubkey = crate::solana::instructions::PROGRAM_ID
        .parse()
        .map_err(|e| ResumeError::Unavailable(format!("program id: {e}")))?;
    let delegation_program: Pubkey = crate::multiplayer::rollup::magicblock::DELEGATION_PROGRAM_ID
        .parse()
        .map_err(|e| ResumeError::Unavailable(format!("delegation program id: {e}")))?;
    let pda = crate::multiplayer::solana::wager_recovery::game_pda(&program_id, game_id);

    let base = RpcClient::new_with_commitment(rpc_url, CommitmentConfig::confirmed());
    let account = base
        .get_account_with_commitment(&pda, CommitmentConfig::confirmed())
        .map_err(|e| ResumeError::Unavailable(format!("base account: {e}")))?
        .value
        .ok_or_else(|| ResumeError::NotInProgress("game account closed".into()))?;
    if account.executable || (account.owner != program_id && account.owner != delegation_program) {
        return Err(ResumeError::InvalidLog(format!(
            "game account owned by unexpected program {}",
            account.owner
        )));
    }
    let base_game = crate::multiplayer::solana::wager_recovery::decode_game(&account.data)
        .ok_or_else(|| ResumeError::InvalidLog("game account undecodable".into()))?;

    let is_delegated = account.owner == delegation_program || base_game.is_delegated;
    // While delegated the base copy is frozen; the ER copy is authoritative.
    let game = if is_delegated {
        let er_url = crate::multiplayer::network::vps::game::game_er_endpoint(game_id)
            .map_err(ResumeError::Unavailable)?;
        let er = RpcClient::new_with_commitment(er_url, CommitmentConfig::confirmed());
        let live = er
            .get_account(&pda)
            .map_err(|e| ResumeError::Unavailable(format!("rollup account: {e}")))?;
        if live.owner != program_id || live.executable {
            return Err(ResumeError::Unavailable(
                "live game is returning to the base layer".into(),
            ));
        }
        crate::multiplayer::solana::wager_recovery::decode_game(&live.data)
            .ok_or_else(|| ResumeError::InvalidLog("rollup game account undecodable".into()))?
    } else {
        base_game
    };
    if game.game_id != game_id {
        return Err(ResumeError::InvalidLog(
            "game ID does not match the requested PDA".into(),
        ));
    }

    let mut events = crate::multiplayer::vps_client::fetch_game_events(game_id)
        .map_err(ResumeError::Unavailable)?;
    let journal = crate::multiplayer::network::braid_transport::open_journal(
        &crate::multiplayer::vps_client::vps_base(),
        &game_id.to_string(),
    )
    .map_err(|e| ResumeError::Unavailable(format!("pending game events: {e}")))?;
    let pending = journal
        .pending()
        .map_err(|e| ResumeError::Unavailable(format!("pending game events: {e}")))?;
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
        if write.sender == wallet.to_string()
            && matches!(write.message, ChessMessage::Move(_))
            && known.insert(write.version)
        {
            events.push(write.message);
        }
    }
    let plan = plan_resume(&game, game.move_count, is_delegated, &wallet, &events)?;
    let next_nonce = if is_delegated {
        Some(
            crate::multiplayer::vps_client::vps_fetch_move_nonce(game_id)
                .map_err(ResumeError::Unavailable)?,
        )
    } else {
        None
    };
    // Best effort: without them the game still resumes, it just cannot watch for
    // the opponent until they show up on the mesh.
    let nodes = crate::multiplayer::vps_client::fetch_game_nodes(&game_id.to_string())
        .unwrap_or_else(|e| {
            warn!("[RESUME] could not look up the opponent node: {e}");
            None
        });
    Ok(ResumeInputs {
        plan,
        next_nonce,
        nodes,
    })
}
#[derive(Debug, Clone, Default, PartialEq)]
pub enum ResumeStatus {
    #[default]
    Idle,
    Checking(u64),
    Failed {
        game_id: u64,
        message: String,
    },
    Resumed(u64),
}

#[derive(Resource, Default)]
pub struct ResumeState {
    pub status: ResumeStatus,
    rx: Option<oneshot::Receiver<Result<ResumeInputs, ResumeError>>>,
}

impl ResumeState {
    /// Start verifying `game_id` for `wallet`. Ignored while a check runs.
    pub fn request(&mut self, game_id: u64, wallet: Pubkey, rpc_url: String) {
        if self.rx.is_some() {
            return;
        }
        let (tx, rx) = oneshot::channel();
        crate::core::net_task::spawn_detached("resume", move || {
            let _ = tx.send(gather(game_id, wallet, rpc_url));
        });
        self.rx = Some(rx);
        self.status = ResumeStatus::Checking(game_id);
    }

    pub fn is_checking(&self) -> bool {
        self.rx.is_some()
    }
}

#[derive(SystemParam)]
pub struct ResumeTargets<'w> {
    session: SessionTargets<'w>,
    sync: ResMut<'w, crate::multiplayer::solana::addon::SolanaGameSync>,
    competitive: ResMut<'w, crate::multiplayer::solana::addon::CompetitiveMatchState>,
    rollup: ResMut<'w, crate::multiplayer::rollup::manager::EphemeralRollupManager>,
    bridge: ResMut<'w, crate::multiplayer::rollup::bridge::RollupNetworkBridge>,
}

pub(crate) fn poll_resume(
    mut resume: ResMut<ResumeState>,
    mut t: ResumeTargets,
    mut go: SessionTransitions,
) {
    let Some(rx) = resume.rx.as_mut() else {
        return;
    };
    let result = match rx.try_recv() {
        Ok(result) => result,
        Err(oneshot::error::TryRecvError::Empty) => return,
        Err(oneshot::error::TryRecvError::Closed) => Err(ResumeError::Unavailable(
            "resume check stopped unexpectedly".into(),
        )),
    };
    let game_id = match resume.status {
        ResumeStatus::Checking(id) => id,
        _ => 0,
    };
    resume.rx = None;
    match result {
        Ok(inputs) => {
            let peer = inputs
                .nodes
                .as_ref()
                .zip(t.session.local_node_id())
                .and_then(|(nodes, local)| opponent_node(nodes, &local));
            apply_plan(&inputs.plan, inputs.next_nonce, peer, &mut t, &mut go);
            resume.status = ResumeStatus::Resumed(inputs.plan.game_id);
        }
        Err(e) => {
            warn!("[RESUME] game {game_id} not resumed: {e:?}");
            resume.status = ResumeStatus::Failed {
                game_id,
                message: e.player_message(),
            };
        }
    }
}

fn apply_plan(
    plan: &ResumePlan,
    next_nonce: Option<u64>,
    peer_node_id: Option<String>,
    t: &mut ResumeTargets,
    go: &mut SessionTransitions,
) {
    let game_id = plan.game_id;
    let replay = MoveReplay {
        fen: plan.fen.clone(),
        my_moves: plan.my_moves,
        opponent_moves: plan.opponent_moves,
        my_head: plan.my_head.clone(),
        opponent_head: plan.opponent_head.clone(),
        applied_versions: plan.applied_versions.clone(),
        checkpoint_fen: plan.chain_fen.clone(),
    };
    // Both players' presence was proven on chain when the game went Active.
    restore_session(
        &RestoredSession {
            session_game_id: game_id.to_string(),
            game_id,
            my_color: plan.my_color,
            replay: &replay,
            last_move_uci: plan.last_move_uci.clone(),
            wager_sol: plan.wager_lamports as f64 / 1_000_000_000.0,
            base_time_seconds: plan.base_time_seconds,
            increment_seconds: plan.increment_seconds,
            time_control: None,
            peer_node_id,
        },
        &mut t.session,
        go,
    );

    t.sync.game_id = Some(game_id);
    t.sync.wager_amount = plan.wager_lamports;
    // Only a delegated game gates input on delegation; resume observes it
    // (joiner path) and never delegates again.
    t.sync.requires_delegation = plan.is_delegated;
    t.competitive.game_id = Some(game_id);
    t.competitive.wager_lamports = plan.wager_lamports;
    t.competitive.stake_amount = plan.wager_lamports;
    t.competitive.active = true;

    t.rollup.assign_game(game_id);
    t.rollup.is_creator = false;
    t.rollup.committed_fen = plan.chain_fen.clone();
    t.rollup.committed_turn = plan.chain_move_count;
    for (uci, fen_after) in &plan.unrecorded {
        t.rollup.add_local_move(uci.clone(), fen_after.clone());
    }
    match next_nonce {
        Some(n) => t.bridge.set_move_nonce(n),
        None if plan.is_delegated => t.bridge.request_nonce_resync(game_id),
        None => {}
    }

    info!(
        "[RESUME] Game {game_id} resumed as {:?}: {} moves ({} recorded on chain, {} re-queued)",
        plan.my_color,
        plan.my_moves + plan.opponent_moves,
        plan.chain_move_count,
        plan.unrecorded.len()
    );
}

fn clear_resume_on_exit(mut resume: ResMut<ResumeState>) {
    if matches!(resume.status, ResumeStatus::Resumed(_)) {
        resume.status = ResumeStatus::Idle;
    }
}

/// The main-menu offer to resume a wagered game that is still running. The Solana
/// lobby shows the same offer on its own screen; this one is for the landing screen,
/// where someone who restarted the app after a crash will be.
fn wagered_resume_prompt(
    mut contexts: bevy_egui::EguiContexts,
    mut lobby: ResMut<crate::multiplayer::solana::lobby::SolanaLobbyState>,
    mut resume: ResMut<ResumeState>,
    solana: Option<Res<crate::multiplayer::solana::integration::state::SolanaIntegrationState>>,
) {
    use bevy_egui::egui;

    let Some(game_id) = lobby.rejoin_game_id else {
        return;
    };
    let Ok(ctx) = contexts.ctx_mut() else { return };
    let wallet = solana.as_ref().and_then(|s| s.wallet_pubkey);
    let checking = resume.is_checking();
    let failure = match &resume.status {
        ResumeStatus::Failed {
            game_id: id,
            message,
        } if *id == game_id => Some(message.clone()),
        _ => None,
    };

    let mut rejoin = false;
    let mut dismiss = false;
    egui::Window::new("wagered_resume_prompt")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_TOP, [0.0, 16.0])
        .show(ctx, |ui| {
            ui.label(
                egui::RichText::new(format!(
                    "Game {game_id} is still in progress. Its clock keeps running."
                ))
                .strong(),
            );
            if let Some(message) = &failure {
                ui.colored_label(egui::Color32::from_rgb(255, 160, 120), message);
            }
            ui.horizontal(|ui| {
                if checking {
                    ui.spinner();
                    ui.label("Verifying game…");
                } else if ui
                    .add_enabled(wallet.is_some(), egui::Button::new("Resume game"))
                    .clicked()
                {
                    rejoin = true;
                }
                if ui.button("Dismiss").clicked() {
                    dismiss = true;
                }
            });
        });

    if rejoin {
        if let Some(wallet) = wallet {
            resume.request(game_id, wallet, lobby.cached_rpc_url.clone());
        }
    }
    if dismiss {
        lobby.rejoin_game_id = None;
    }
}

pub struct ResumePlugin;

impl Plugin for ResumePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ResumeState>()
            .add_systems(
                Update,
                poll_resume.in_set(crate::core::schedule::SessionSet::Match),
            )
            .add_systems(OnExit(crate::core::GameState::InGame), clear_resume_on_exit)
            .add_systems(
                bevy_egui::EguiPrimaryContextPass,
                wagered_resume_prompt.run_if(in_state(crate::core::MenuState::Main)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xfchess_game::state::{GameResult, GameType, MatchType};

    const E4: &str = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1";
    const E4E5: &str = "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e6 0 2";
    const NF3: &str = "rnbqkbnr/pppp1ppp/8/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R b KQkq - 1 2";

    fn game(white: Pubkey, black: Pubkey, status: GameStatus, move_count: u16) -> Game {
        Game {
            game_id: 9,
            white,
            black,
            status,
            last_move_timestamp: 0,
            fees_advanced: 0,
            fee_payer: Pubkey::new_unique(),
            result: GameResult::None,
            board_state: [0; 68],
            move_count,
            halfmove_clock: 0,
            turn: move_count + 1,
            created_at: 0,
            updated_at: 0,
            wager_amount: 5_000_000,
            wager_token: None,
            game_type: GameType::PvP,
            match_type: MatchType::Rated,
            country_fee: 0,
            base_time_seconds: 300,
            increment_seconds: 2,
            bump: 255,
            is_delegated: true,
            tournament_id: None,
            nonce: move_count as u64,
            draw_offered_by: None,
        }
    }

    fn mv(uci: &str, fen: &str, n: u32) -> ChessMessage {
        ChessMessage::Move(MovePayload::from_uci(uci, fen, n, "node"))
    }

    fn log() -> Vec<ChessMessage> {
        vec![mv("e2e4", E4, 1), mv("e7e5", E4E5, 1), mv("g1f3", NF3, 2)]
    }

    #[test]
    fn white_resumes_with_own_lane_opponent_lane_and_unrecorded_tail(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (w, b) = (Pubkey::new_unique(), Pubkey::new_unique());
        let plan = plan_resume(&game(w, b, GameStatus::Active, 2), 2, true, &w, &log())
            .map_err(|error| format!("{error:?}"))?;
        assert_eq!(plan.my_color, PieceColor::White);
        assert_eq!(plan.fen, NF3);
        assert_eq!((plan.my_moves, plan.opponent_moves), (2, 1));
        assert_eq!(plan.my_head, braid_chess::version_hash(NF3, 2));
        assert_eq!(plan.opponent_head, braid_chess::version_hash(E4E5, 1));
        assert_eq!(plan.applied_versions.len(), 3);
        assert_eq!(plan.chain_fen, E4E5);
        assert_eq!(plan.unrecorded, vec![("g1f3".to_string(), NF3.to_string())]);
        assert_eq!(plan.last_move_uci.as_deref(), Some("g1f3"));
        assert_eq!((plan.base_time_seconds, plan.increment_seconds), (300, 2));

        Ok(())
    }

    #[test]
    fn black_resumes_from_its_own_perspective() -> Result<(), Box<dyn std::error::Error>> {
        let (w, b) = (Pubkey::new_unique(), Pubkey::new_unique());
        let plan = plan_resume(&game(w, b, GameStatus::Active, 3), 3, true, &b, &log())
            .map_err(|error| format!("{error:?}"))?;
        assert_eq!(plan.my_color, PieceColor::Black);
        assert_eq!((plan.my_moves, plan.opponent_moves), (1, 2));
        assert!(plan.unrecorded.is_empty());
        assert_eq!(plan.last_move_uci.as_deref(), Some("g1f3"));
        assert_eq!(plan.chain_fen, NF3);

        Ok(())
    }

    #[test]
    fn zero_move_game_resumes_at_the_start_position() -> Result<(), Box<dyn std::error::Error>> {
        let (w, b) = (Pubkey::new_unique(), Pubkey::new_unique());
        let plan = plan_resume(&game(w, b, GameStatus::Active, 0), 0, false, &b, &[])
            .map_err(|error| format!("{error:?}"))?;
        assert_eq!(plan.fen, START_FEN);
        assert_eq!(plan.my_head, "0");
        assert_eq!(plan.last_move_uci, None);

        Ok(())
    }

    #[test]
    fn another_wallet_cannot_resume_the_game() {
        let (w, b) = (Pubkey::new_unique(), Pubkey::new_unique());
        let err = plan_resume(
            &game(w, b, GameStatus::Active, 2),
            2,
            true,
            &Pubkey::new_unique(),
            &log(),
        );
        assert_eq!(err, Err(ResumeError::NotParticipant));
    }

    #[test]
    fn finished_resigned_or_cancelled_games_report_their_end_instead_of_resuming() {
        let (w, b) = (Pubkey::new_unique(), Pubkey::new_unique());
        assert!(matches!(
            plan_resume(&game(w, b, GameStatus::Finished, 3), 3, true, &w, &log()),
            Err(ResumeError::Ended(_))
        ));
        assert!(matches!(
            plan_resume(&game(w, b, GameStatus::Cancelled, 0), 0, false, &w, &[]),
            Err(ResumeError::NotInProgress(_))
        ));
        let mut resigned = log();
        resigned.push(ChessMessage::Resign {
            player: "black".into(),
        });
        assert!(matches!(
            plan_resume(&game(w, b, GameStatus::Active, 3), 3, true, &w, &resigned),
            Err(ResumeError::Ended(_))
        ));
    }

    #[test]
    fn log_behind_chain_or_illegal_history_is_refused() {
        let (w, b) = (Pubkey::new_unique(), Pubkey::new_unique());
        assert_eq!(
            plan_resume(&game(w, b, GameStatus::Active, 4), 4, true, &w, &log()),
            Err(ResumeError::LogBehindChain { log: 3, chain: 4 })
        );
        let forged = vec![mv("e2e5", E4, 1)];
        assert!(matches!(
            plan_resume(&game(w, b, GameStatus::Active, 0), 0, true, &w, &forged),
            Err(ResumeError::InvalidLog(_))
        ));
        let wrong_ply = vec![mv("e2e4", E4E5, 1)];
        assert!(matches!(
            plan_resume(&game(w, b, GameStatus::Active, 0), 0, true, &w, &wrong_ply),
            Err(ResumeError::InvalidLog(_))
        ));
    }

    #[test]
    fn session_info_and_draw_offers_do_not_block_resume() {
        let (w, b) = (Pubkey::new_unique(), Pubkey::new_unique());
        let mut events = vec![ChessMessage::SessionInfo {
            player_pubkey: w.to_string(),
            session_pubkey: "s".into(),
            signing_pubkey: "g".into(),
            expires_at: 0,
        }];
        events.extend(log());
        events.push(ChessMessage::OfferDraw {
            player: "white".into(),
            ply: 3,
            at_ms: 1,
        });
        events.push(ChessMessage::DeclineDraw {
            player: "black".into(),
            ply: 3,
            at_ms: 2,
        });
        assert!(plan_resume(&game(w, b, GameStatus::Active, 3), 3, true, &w, &events).is_ok());
    }
}
