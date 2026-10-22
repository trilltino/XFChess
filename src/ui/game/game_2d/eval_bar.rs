use super::*;
use crate::game::ai::systems::StockfishProcess;
use xfchess_stockfish::{EnginePosition, Score, SearchRequest, SearchTicket};

#[derive(Resource, Default)]
pub struct EvalHistory {
    /// Missing values are pending or unavailable, never a fabricated zero score.
    pub scores: Vec<Option<i16>>,
    positions: Vec<String>,
    mates: Vec<Option<i32>>,
    failed: std::collections::HashSet<usize>,
    worker: Option<StockfishProcess>,
    pending: Option<(usize, String, SearchTicket)>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EvaluationStatus {
    #[default]
    Pending,
    Ready,
    Unavailable,
}

#[derive(Resource, Default)]
pub struct EvalBarState {
    pub score: i16,
    pub mate: Option<i32>,
    pub visible: bool,
    pub status: EvaluationStatus,
}

impl EvalBarState {
    pub fn white_fraction(&self) -> f32 {
        let clamped = self.score.clamp(-500, 500) as f32;
        (clamped + 500.0) / 1000.0
    }
}

pub fn reset_evaluation(mut history: ResMut<EvalHistory>, mut eval: ResMut<EvalBarState>) {
    *history = EvalHistory::default();
    *eval = EvalBarState::default();
}

pub fn update_eval_bar(
    history: Res<crate::game::resources::MoveHistory>,
    engine: Res<crate::engine::board_state::ChessEngine>,
    mut eval: ResMut<EvalBarState>,
    mut evaluations: ResMut<EvalHistory>,
    game_mode: Res<GameMode>,
    replay: Option<Res<crate::game::replay::PgnReplayState>>,
    pgn: Option<Res<crate::game::replay::ParsedPgnGameResource>>,
    settings: Res<crate::core::GameSettings>,
) {
    let restricted = matches!(
        *game_mode,
        GameMode::MultiplayerCompetitive | GameMode::Puzzle
    ) || pgn
        .as_ref()
        .is_some_and(|p| p.puzzle_mode && !p.puzzle_revealed);
    eval.visible = settings.show_eval_bar && !restricted;
    let graph =
        *game_mode == GameMode::PgnReplay && pgn.as_ref().is_some_and(|p| p.show_eval_graph);
    if restricted || (!eval.visible && !graph) {
        evaluations.pending = None;
        evaluations.worker = None;
        if restricted {
            *evaluations = EvalHistory::default();
            eval.score = 0;
            eval.mate = None;
            eval.status = EvaluationStatus::Unavailable;
        }
        return;
    }
    let positions = if *game_mode == GameMode::PgnReplay {
        replay
            .as_ref()
            .map(|r| r.fen_snapshots.clone())
            .unwrap_or_default()
    } else {
        let mut game = xfchess_rules::Game::default();
        let mut positions = Vec::with_capacity(history.len());
        for record in history.iter() {
            if game.play_uci(&record.uci()).is_err() {
                positions.clear();
                break;
            }
            positions.push(game.fen());
        }
        if positions.last().map(String::as_str) != Some(engine.current_fen()) {
            positions = vec![engine.current_fen().to_string()];
        }
        positions
    };
    if positions != evaluations.positions {
        let common = positions
            .iter()
            .zip(&evaluations.positions)
            .take_while(|(a, b)| a == b)
            .count();
        evaluations.pending = None;
        evaluations.scores.truncate(common);
        evaluations.mates.truncate(common);
        evaluations.failed.retain(|ply| *ply < common);
        evaluations.scores.resize(positions.len(), None);
        evaluations.mates.resize(positions.len(), None);
        evaluations.positions = positions;
    }
    let current = if *game_mode == GameMode::PgnReplay {
        replay.as_ref().map_or(0, |r| r.current_ply)
    } else {
        evaluations.positions.len().saturating_sub(1)
    };
    if current >= evaluations.positions.len() {
        return;
    }
    if let Some((ply, fen, ticket)) = evaluations.pending.take() {
        if let Some(result) = ticket.try_result() {
            if evaluations.positions.get(ply) == Some(&fen) {
                match result {
                    Ok(result) => {
                        if let Some(score) = result.score {
                            evaluations.scores[ply] =
                                Some(score.centipawns().clamp(-30_000, 30_000) as i16);
                            evaluations.mates[ply] = match score {
                                Score::Mate(m) => Some(m),
                                _ => None,
                            };
                        } else {
                            evaluations.failed.insert(ply);
                        }
                    }
                    Err(error) => {
                        warn!("[analysis] Stockfish: {error}");
                        evaluations.failed.insert(ply);
                    }
                }
            }
        } else if evaluations.scores[current].is_some() || ply == current {
            evaluations.pending = Some((ply, fen, ticket));
        }
    }
    eval.score = evaluations.scores[current].unwrap_or(0);
    eval.mate = evaluations.mates[current];
    eval.status = if evaluations.scores[current].is_some() {
        EvaluationStatus::Ready
    } else if evaluations.failed.contains(&current) {
        EvaluationStatus::Unavailable
    } else {
        EvaluationStatus::Pending
    };
    if evaluations.pending.is_some() {
        return;
    }
    let next = if evaluations.scores[current].is_none() && !evaluations.failed.contains(&current) {
        Some(current)
    } else {
        (0..evaluations.positions.len())
            .find(|ply| evaluations.scores[*ply].is_none() && !evaluations.failed.contains(ply))
    };
    let Some(ply) = next else {
        return;
    };
    if evaluations.worker.is_none() {
        match StockfishProcess::start() {
            Ok(worker) => evaluations.worker = Some(worker),
            Err(error) => {
                warn!("[analysis] {error}");
                evaluations.failed.insert(ply);
                eval.status = EvaluationStatus::Unavailable;
                return;
            }
        }
    }
    let fen = evaluations.positions[ply].clone();
    if let Ok(game) = xfchess_rules::Game::from_fen(&fen) {
        if game.legal_moves().is_empty() {
            let score = if game.is_checkmate() {
                if game.white_to_move() {
                    -30_000
                } else {
                    30_000
                }
            } else {
                0
            };
            evaluations.scores[ply] = Some(score);
            evaluations.mates[ply] = if game.is_checkmate() { Some(0) } else { None };
            return;
        }
    }
    let Some(worker) = evaluations.worker.as_ref() else {
        return;
    };
    let ticket = worker.0.search(SearchRequest {
        game_id: 0,
        position: EnginePosition::from_fen(fen.clone()),
        movetime_ms: 100,
        depth: None,
        strength_elo: None,
    });
    evaluations.pending = Some((ply, fen, ticket));
}

pub fn sync_eval_bar_visibility(
    settings: Res<crate::core::settings::GameSettings>,
    game_mode: Res<GameMode>,
    pgn: Option<Res<crate::game::replay::ParsedPgnGameResource>>,
    mut eval: ResMut<EvalBarState>,
) {
    eval.visible = settings.show_eval_bar
        && !matches!(
            *game_mode,
            GameMode::MultiplayerCompetitive | GameMode::Puzzle
        )
        && !pgn
            .as_ref()
            .is_some_and(|p| p.puzzle_mode && !p.puzzle_revealed);
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn competitive_games_and_puzzles_clear_analysis_without_starting_stockfish(
    ) -> Result<(), Box<dyn std::error::Error>> {
        for mode in [GameMode::MultiplayerCompetitive, GameMode::Puzzle] {
            let mut world = World::new();
            world.insert_resource(mode);
            world.insert_resource(crate::core::GameSettings {
                show_eval_bar: true,
                ..Default::default()
            });
            world.init_resource::<crate::game::resources::MoveHistory>();
            world.init_resource::<crate::engine::board_state::ChessEngine>();
            world.insert_resource(EvalBarState {
                score: 200,
                visible: true,
                ..Default::default()
            });
            world.insert_resource(EvalHistory {
                scores: vec![Some(200)],
                ..Default::default()
            });
            world
                .run_system_once(update_eval_bar)
                .map_err(|error| format!("{error:?}"))?;
            let history = world.resource::<EvalHistory>();
            assert!(history.scores.is_empty());
            assert!(history.worker.is_none());
            let eval = world.resource::<EvalBarState>();
            assert!(!eval.visible);
            assert_eq!(eval.score, 0);
        }

        Ok(())
    }
}
