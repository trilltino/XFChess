use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use super::{CapturedPieces, CurrentGamePhase, CurrentTurn, GameOverState};
use crate::engine::board_state::ChessEngine;
use crate::game::resources::player::selection::Selection;

#[derive(SystemParam)]
pub struct GameStateParams<'w> {
    pub current_turn: Res<'w, CurrentTurn>,
    pub game_phase: Res<'w, CurrentGamePhase>,
    pub game_over: Res<'w, GameOverState>,
    pub captured: Res<'w, CapturedPieces>,
    pub selection: ResMut<'w, Selection>,
    pub engine: ResMut<'w, ChessEngine>,
}

#[derive(SystemParam)]
pub struct AIParams<'w> {
    pub ai_config: Res<'w, crate::game::ai::ChessAIResource>,
    pub pending_ai: Option<Res<'w, crate::game::ai::PendingAIMove>>,
    pub ai_stats: Res<'w, crate::game::ai::AIStatistics>,
}
