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

use crate::engine::board_state::ChessEngine;
use crate::multiplayer::systems::fen_ply;
use crate::rendering::pieces::PieceColor;

pub const START_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

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
    pub wager_lamports: u64,
    pub base_time_seconds: u64,
    pub increment_seconds: u16,
    pub is_delegated: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ResumeError {
    NotParticipant,
    NotInProgress(String),
    Ended(String),
    LogBehindChain { log: usize, chain: u16 },
    LogTooFarAhead { log: usize, chain: u16 },
    InvalidLog(String),
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

fn mover_of(fen_before: &str) -> PieceColor {
    if fen_before.split_whitespace().nth(1) == Some("b") {
        PieceColor::Black
    } else {
        PieceColor::White
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

    let mut engine = ChessEngine::default();
    let mut prev = START_FEN.to_string();
    let mut my_moves = 0u32;
    let mut opponent_moves = 0u32;
    let mut my_head = "0".to_string();
    let mut opponent_head = "0".to_string();
    let mut applied_versions = Vec::with_capacity(moves.len());
    let mut chain_fen = START_FEN.to_string();
    for (i, m) in moves.iter().enumerate() {
        engine
            .set_from_fen(&prev)
            .map_err(|e| ResumeError::InvalidLog(format!("position before move {}: {e}", i + 1)))?;
        engine.rebuild_legal_move_cache();
        if !engine.is_move_legal_by_uci(&m.uci) {
            return Err(ResumeError::InvalidLog(format!(
                "move {} ({}) is illegal",
                i + 1,
                m.uci
            )));
        }
        if fen_ply(&m.fen_after) != Some(i as u64 + 1) {
            return Err(ResumeError::InvalidLog(format!(
                "move {} reports the wrong ply ({})",
                i + 1,
                m.fen_after
            )));
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
        if i + 1 == chain {
            chain_fen = prev.clone();
        }
    }

    Ok(ResumePlan {
        game_id: game.game_id,
        my_color,
        fen: prev,
        my_moves,
        opponent_moves,
        my_head,
        opponent_head,
        applied_versions,
        chain_fen,
        chain_move_count,
        unrecorded: moves[chain..]
            .iter()
            .map(|m| (m.uci.clone(), m.fen_after.clone()))
            .collect(),
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
}

fn gather(
    game_id: u64,
    wallet: Pubkey,
    rpc_url: String,
    er_url: String,
) -> Result<ResumeInputs, ResumeError> {
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
    if account.owner != program_id && account.owner != delegation_program {
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
        let er = RpcClient::new_with_commitment(er_url, CommitmentConfig::confirmed());
        let data = er
            .get_account_data(&pda)
            .map_err(|e| ResumeError::Unavailable(format!("rollup account: {e}")))?;
        crate::multiplayer::solana::wager_recovery::decode_game(&data)
            .ok_or_else(|| ResumeError::InvalidLog("rollup game account undecodable".into()))?
    } else {
        base_game
    };

    let events = crate::multiplayer::vps_client::fetch_game_events(game_id)
        .map_err(ResumeError::Unavailable)?;
    let plan = plan_resume(&game, game.move_count, is_delegated, &wallet, &events)?;
    let next_nonce = if is_delegated {
        Some(
            crate::multiplayer::vps_client::vps_fetch_move_nonce(game_id)
                .map_err(ResumeError::Unavailable)?,
        )
    } else {
        None
    };
    Ok(ResumeInputs { plan, next_nonce })
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
    /// Position to apply to the engine once the resumed board has spawned.
    pending_engine_fen: Option<String>,
}

impl ResumeState {
    /// Start verifying `game_id` for `wallet`. Ignored while a check runs.
    pub fn request(&mut self, game_id: u64, wallet: Pubkey, rpc_url: String, er_url: String) {
        if self.rx.is_some() {
            return;
        }
        let (tx, rx) = oneshot::channel();
        std::thread::spawn(move || {
            let _ = tx.send(gather(game_id, wallet, rpc_url, er_url));
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
    online_session: ResMut<'w, crate::multiplayer::network::online_game_session::OnlineGameSession>,
    network_config: Res<'w, crate::multiplayer::types::NetworkConfig>,
    network_state: Option<Res<'w, crate::multiplayer::OnlineNetworkState>>,
    p2p: ResMut<'w, crate::multiplayer::network::p2p::P2PConnectionState>,
    sync: ResMut<'w, crate::multiplayer::solana::addon::SolanaGameSync>,
    competitive: ResMut<'w, crate::multiplayer::solana::addon::CompetitiveMatchState>,
    rollup: ResMut<'w, crate::multiplayer::rollup::manager::EphemeralRollupManager>,
    bridge: ResMut<'w, crate::multiplayer::rollup::bridge::RollupNetworkBridge>,
    causal: ResMut<'w, crate::multiplayer::types::CausalChainState>,
    pending: ResMut<'w, crate::multiplayer::types::PendingMoveBuffer>,
    barrier: ResMut<'w, crate::multiplayer::types::OnlineStartBarrier>,
    resume_board: ResMut<'w, crate::rendering::pieces::ResumeBoard>,
    time_control: ResMut<'w, crate::game::resources::active_time_control::ActiveTimeControl>,
}

#[derive(SystemParam)]
pub struct ResumeTransitions<'w> {
    ai_config: ResMut<'w, crate::game::ai::resource::ChessAIResource>,
    core_mode: ResMut<'w, crate::core::GameMode>,
    next_state: ResMut<'w, NextState<crate::core::GameState>>,
    menu_state: ResMut<'w, NextState<crate::core::MenuState>>,
    started: MessageWriter<'w, crate::game::events::GameStartedEvent>,
}

fn poll_resume(mut resume: ResMut<ResumeState>, mut t: ResumeTargets, mut go: ResumeTransitions) {
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
            apply_plan(&inputs.plan, inputs.next_nonce, &mut t, &mut go);
            resume.pending_engine_fen = Some(inputs.plan.fen.clone());
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
    t: &mut ResumeTargets,
    go: &mut ResumeTransitions,
) {
    use crate::multiplayer::network::online_game_session::{numeric_game_id, start_session};

    let game_id = plan.game_id;
    let wager_sol = plan.wager_lamports as f64 / 1_000_000_000.0;
    if let Some(network_state) = t.network_state.as_deref() {
        start_session(
            &mut t.online_session,
            t.network_config.vps_base_url.clone(),
            game_id.to_string(),
            wager_sol,
            network_state,
        );
    }
    // Continue this player's own Braid/gossip lane where the log ends.
    t.online_session.next_move_number = plan.my_moves + 1;
    t.online_session.next_nonce = plan.my_moves as u64 + 1;
    t.online_session.last_published_move_version = plan.my_head.clone();

    let net_game_id = numeric_game_id(&game_id.to_string());
    // Log replay on resubscribe must not re-apply moves the snapshot already has.
    t.causal
        .applied_versions
        .entry(net_game_id)
        .or_default()
        .extend(plan.applied_versions.iter().cloned());
    t.causal.resume_seeds.insert(
        net_game_id,
        (plan.opponent_moves as u64, plan.opponent_head.clone()),
    );
    t.pending.sequencers.insert(
        net_game_id,
        crate::multiplayer::network::reorder::NonceSequencer::starting_at(
            plan.opponent_moves as u64 + 1,
            crate::multiplayer::types::PendingMoveBuffer::MAX_BUFFERED,
        ),
    );
    // Both players' presence was proven on chain when the game went Active.
    *t.barrier = crate::multiplayer::types::OnlineStartBarrier {
        game_id: net_game_id,
        local_ready: true,
        remote_ready: true,
        start_confirmed: true,
        ready_sent: true,
        start_sent: true,
    };

    t.p2p.is_host = plan.my_color == PieceColor::White;
    t.p2p.player_color = Some(plan.my_color);
    t.p2p.status = crate::multiplayer::network::p2p::P2PConnectionStatus::InGame;

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

    t.resume_board.fen = Some(plan.fen.clone());
    if plan.base_time_seconds > 0 {
        t.time_control.control = crate::game::time_control::TimeControl::Custom {
            base_seconds: plan.base_time_seconds.min(u32::MAX as u64) as u32,
            increment_seconds: plan.increment_seconds,
        };
    }
    crate::multiplayer::network::game_id_store::set(game_id);

    go.ai_config.mode = crate::game::ai::resource::GameMode::Multiplayer;
    *go.core_mode = crate::core::GameMode::OnlineMultiplayer;
    go.started
        .write(crate::game::events::GameStartedEvent { game_id });
    go.next_state.set(crate::core::GameState::InGame);
    go.menu_state.set(crate::core::MenuState::Main);
    info!(
        "[RESUME] Game {game_id} resumed as {:?}: {} moves ({} recorded on chain, {} re-queued)",
        plan.my_color,
        plan.my_moves + plan.opponent_moves,
        plan.chain_move_count,
        plan.unrecorded.len()
    );
}

/// Once the resumed position's pieces exist, put the engine and turn state on
/// the same position (entering `InGame` reset them to the start position).
fn apply_resume_board(
    mut resume: ResMut<ResumeState>,
    pieces_spawned: Res<crate::rendering::pieces::PiecesSpawned>,
    mut resume_board: ResMut<crate::rendering::pieces::ResumeBoard>,
    mut engine: ResMut<ChessEngine>,
    mut current_turn: ResMut<crate::game::resources::CurrentTurn>,
    mut turn_context: ResMut<crate::game::resources::TurnStateContext>,
) {
    if !pieces_spawned.spawned {
        return;
    }
    let Some(fen) = resume.pending_engine_fen.take() else {
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

fn clear_resume_on_exit(
    mut resume: ResMut<ResumeState>,
    mut resume_board: ResMut<crate::rendering::pieces::ResumeBoard>,
) {
    resume.pending_engine_fen = None;
    if matches!(resume.status, ResumeStatus::Resumed(_)) {
        resume.status = ResumeStatus::Idle;
    }
    resume_board.fen = None;
}

pub struct ResumePlugin;

impl Plugin for ResumePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ResumeState>()
            .add_systems(Update, poll_resume)
            .add_systems(
                Update,
                apply_resume_board.run_if(in_state(crate::core::GameState::InGame)),
            )
            .add_systems(OnExit(crate::core::GameState::InGame), clear_resume_on_exit);
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
    fn white_resumes_with_own_lane_opponent_lane_and_unrecorded_tail() {
        let (w, b) = (Pubkey::new_unique(), Pubkey::new_unique());
        let plan = plan_resume(&game(w, b, GameStatus::Active, 2), 2, true, &w, &log()).unwrap();
        assert_eq!(plan.my_color, PieceColor::White);
        assert_eq!(plan.fen, NF3);
        assert_eq!((plan.my_moves, plan.opponent_moves), (2, 1));
        assert_eq!(plan.my_head, braid_chess::version_hash(NF3, 2));
        assert_eq!(plan.opponent_head, braid_chess::version_hash(E4E5, 1));
        assert_eq!(plan.applied_versions.len(), 3);
        assert_eq!(plan.chain_fen, E4E5);
        assert_eq!(plan.unrecorded, vec![("g1f3".to_string(), NF3.to_string())]);
        assert_eq!((plan.base_time_seconds, plan.increment_seconds), (300, 2));
    }

    #[test]
    fn black_resumes_from_its_own_perspective() {
        let (w, b) = (Pubkey::new_unique(), Pubkey::new_unique());
        let plan = plan_resume(&game(w, b, GameStatus::Active, 3), 3, true, &b, &log()).unwrap();
        assert_eq!(plan.my_color, PieceColor::Black);
        assert_eq!((plan.my_moves, plan.opponent_moves), (1, 2));
        assert!(plan.unrecorded.is_empty());
        assert_eq!(plan.chain_fen, NF3);
    }

    #[test]
    fn zero_move_game_resumes_at_the_start_position() {
        let (w, b) = (Pubkey::new_unique(), Pubkey::new_unique());
        let plan = plan_resume(&game(w, b, GameStatus::Active, 0), 0, false, &b, &[]).unwrap();
        assert_eq!(plan.fen, START_FEN);
        assert_eq!(plan.my_head, "0");
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
        });
        events.push(ChessMessage::DeclineDraw {
            player: "black".into(),
        });
        assert!(plan_resume(&game(w, b, GameStatus::Active, 3), 3, true, &w, &events).is_ok());
    }
}
