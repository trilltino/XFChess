use super::*;

#[derive(Resource, Default)]
pub struct EvalHistory {
    pub scores: Vec<i16>,
    pub(super) cached_game: Option<nimzovich_engine::Game>,
}

#[derive(Resource, Default)]
pub struct EvalBarState {
    pub score: i16,
    pub visible: bool,
}

impl EvalBarState {
    pub fn white_fraction(&self) -> f32 {
        let clamped = self.score.clamp(-500, 500) as f32;
        (clamped + 500.0) / 1000.0
    }
}

pub fn update_eval_bar(
    history: Res<crate::game::resources::MoveHistory>,
    mut eval: ResMut<EvalBarState>,
    mut eval_history: ResMut<EvalHistory>,
    game_mode: Res<crate::core::states::GameMode>,
) {
    use crate::core::states::GameMode;
    if !history.is_changed() {
        return;
    }
    // Hide eval in competitive online (anti-cheat)
    if matches!(*game_mode, GameMode::MultiplayerCompetitive) {
        eval.score = 0;
        eval_history.scores.clear();
        return;
    }
    use crate::game::components::PieceType;
    use nimzovich_engine::{do_move_with_promo, evaluate_position, new_game_no_tt};

    let moves = &history.moves;
    let cached_count = eval_history.scores.len();

    // If history shrank (e.g. new game started), reset cached state.
    if cached_count > moves.len() {
        eval_history.scores.clear();
        eval_history.cached_game = None;
    }

    // Nothing new to evaluate.
    if eval_history.scores.len() == moves.len() {
        return;
    }

    // Use new_game_no_tt for move application and evaluation; no search occurs
    // here, so allocating a transposition table would stall the first move.
    let start = eval_history.scores.len();
    let mut game = eval_history
        .cached_game
        .take()
        .unwrap_or_else(new_game_no_tt);
    for rec in &moves[start..] {
        let src = rec.from.1 as i8 * 8 + rec.from.0 as i8;
        let dst = rec.to.1 as i8 * 8 + rec.to.0 as i8;
        let is_promo = rec.piece_type == PieceType::Pawn && (rec.to.1 == 7 || rec.to.1 == 0);
        let promo: i8 = if is_promo { 5 } else { 0 };
        do_move_with_promo(&mut game, src, dst, true, promo);
        eval_history.scores.push(evaluate_position(&game));
    }
    eval_history.cached_game = Some(game);
    eval.score = eval_history.scores.last().copied().unwrap_or(0);
}

pub fn sync_eval_bar_visibility(
    settings: Res<crate::core::resources::GameSettings>,
    mut eval: ResMut<EvalBarState>,
) {
    eval.visible = settings.show_eval_bar;
}
