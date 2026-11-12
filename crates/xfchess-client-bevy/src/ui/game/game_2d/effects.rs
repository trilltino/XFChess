use super::*;

#[derive(Resource, Default)]
pub struct CheckmateFlashState {
    pub active: bool,
    pub elapsed: f32,
    pub king_sq: (u8, u8),
}

impl CheckmateFlashState {
    pub(super) const DURATION: f32 = 2.0;
    pub fn alpha(&self) -> u8 {
        let t = (self.elapsed / Self::DURATION).min(1.0);
        // Pulses: sin wave fading out
        let pulse = ((t * std::f32::consts::TAU * 3.0).sin() * 0.5 + 0.5) * (1.0 - t);
        (pulse * 200.0) as u8
    }
}

pub fn trigger_checkmate_flash(
    game_over: Res<crate::game::resources::history::game_over::GameOverState>,
    mut flash: ResMut<CheckmateFlashState>,
    pieces: Query<&crate::rendering::pieces::Piece>,
) {
    if flash.active {
        return;
    }
    if !game_over.is_checkmate() {
        return;
    }
    use crate::rendering::pieces::{PieceColor, PieceType};
    // WhiteWon = Black was checkmated → Black king flashes
    let loser_color = match &*game_over {
        crate::game::resources::history::game_over::GameOverState::WhiteWon => PieceColor::Black,
        crate::game::resources::history::game_over::GameOverState::BlackWon => PieceColor::White,
        _ => return,
    };
    for piece in &pieces {
        if piece.piece_type == PieceType::King && piece.color == loser_color {
            flash.active = true;
            flash.elapsed = 0.0;
            flash.king_sq = (piece.x, piece.y);
            return;
        }
    }
}

pub fn tick_checkmate_flash(mut flash: ResMut<CheckmateFlashState>, time: Res<Time>) {
    if !flash.active {
        return;
    }
    flash.elapsed += time.delta_secs();
    if flash.elapsed >= CheckmateFlashState::DURATION {
        flash.active = false;
    }
}

#[derive(Resource)]
pub struct BoardFadeState {
    pub alpha_mult: f32,
    pub target: f32,
}

impl Default for BoardFadeState {
    fn default() -> Self {
        Self {
            alpha_mult: 1.0,
            target: 1.0,
        }
    }
}

pub fn board_fade_system(
    game_over: Res<crate::game::resources::history::game_over::GameOverState>,
    mut fade: ResMut<BoardFadeState>,
    time: Res<Time>,
) {
    if game_over.is_resignation() {
        fade.target = 0.4;
    }
    fade.alpha_mult += (fade.target - fade.alpha_mult) * (time.delta_secs() * 3.0).min(1.0);
}
