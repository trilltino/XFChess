//! The 2D top-down board ([`render`]), with its animations ([`effects`]), eval bar
//! ([`eval_bar`]) and colour themes ([`theme`]).

use crate::core::state::GameMode;
use crate::game::components::FadingCapture;
use crate::game::resources::premove::Premove;
use crate::game::resources::{CurrentTurn, Players};
use crate::game::systems::camera::get_is_black_view;
use crate::game::systems::shared::CapturedTarget;
use crate::game::view_mode::ViewMode;
use crate::input::board::{
    can_move_color, clear_selection_state, is_human_turn, queue_board_move, queue_premove_click,
    try_select_piece, InputSystemParams,
};
use crate::motion::{BoardEpoch, PieceMotion};
use crate::rendering::pieces::{PieceColor, PieceType};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_egui::egui;
use std::collections::HashMap;

mod effects;
mod eval_bar;
mod render;
mod theme;
pub use effects::*;
pub use eval_bar::*;
pub use render::*;
pub use theme::*;

#[derive(SystemParam)]
pub struct Board2DExtras<'w> {
    pub eval_bar: Res<'w, EvalBarState>,
    pub arrows: ResMut<'w, BoardArrows>,
    pub keyboard: Res<'w, ButtonInput<KeyCode>>,
    pub sounds: Option<Res<'w, crate::game::resources::GameSounds>>,
    pub settings: Res<'w, crate::core::GameSettings>,
    pub drag: ResMut<'w, DragState2D>,
    pub premove: ResMut<'w, Premove>,
    pub epoch: Res<'w, BoardEpoch>,
    pub promotion: Res<'w, crate::game::resources::PendingPromotion>,
    pub promotion_writer:
        bevy::prelude::MessageWriter<'w, crate::game::resources::PromotionSelected>,
    pub focus: ResMut<'w, BoardFocus>,
    pub cm_flash: Res<'w, CheckmateFlashState>,
    pub board_fade: Res<'w, BoardFadeState>,
    pub player_bars: Res<'w, crate::ui::game::player_bar::PlayerBarsCache>,
    pub bar_layout: ResMut<'w, crate::ui::game::player_bar::PlayerBarLayoutCache>,
}

// The local is_black_view function is removed in favor of the shared helper in camera.rs

fn board_to_screen(file: u8, rank: u8, black_view: bool, square_size: f32) -> egui::Vec2 {
    let (sx, sy) = if black_view {
        (7 - file, rank)
    } else {
        (file, 7 - rank)
    };
    egui::Vec2::new(sx as f32 * square_size, sy as f32 * square_size)
}

#[derive(Resource, Default)]
pub struct DragState2D {
    pub dragging: bool,
    pub from: (u8, u8),
    pub cursor_pos: egui::Pos2,
    pub piece: Option<(PieceType, PieceColor)>,
}

/// A pointer gesture belongs to one view and one board generation. Release
/// events may arrive while egui's board is hidden, so cleanup runs in ECS.
pub fn reset_stale_drag(
    view: Res<ViewMode>,
    epoch: Res<BoardEpoch>,
    mut drag: ResMut<DragState2D>,
    mut previous: Local<Option<(ViewMode, u64)>>,
) {
    let current = (*view, epoch.0);
    if *view != ViewMode::Standard2D || previous.is_none_or(|old| old != current) {
        *drag = DragState2D::default();
    }
    *previous = Some(current);
}

#[derive(Resource, Default)]
pub struct Board2DLayout {
    pub rect: Option<egui::Rect>,
}

#[derive(Resource)]
pub struct BoardFocus {
    pub active: bool,
    pub cursor: (u8, u8),
}

impl Default for BoardFocus {
    fn default() -> Self {
        Self {
            active: false,
            cursor: (4, 1),
        }
    }
}

#[derive(Resource, Default)]
pub struct BoardArrows {
    pub arrows: Vec<(u8, u8, u8, u8, u8)>,
    pub drag_from: Option<(u8, u8)>,
}

#[cfg(test)]
mod drag_lifecycle_tests {
    use super::*;

    #[test]
    fn hidden_views_and_rebuilt_boards_cancel_pointer_gestures() {
        let mut app = App::new();
        app.insert_resource(ViewMode::Standard2D)
            .init_resource::<BoardEpoch>()
            .init_resource::<DragState2D>();
        app.add_systems(Update, reset_stale_drag);
        app.update();
        for change_view in [true, false] {
            app.world_mut().resource_mut::<DragState2D>().dragging = true;
            app.world_mut().resource_mut::<DragState2D>().piece =
                Some((PieceType::Pawn, PieceColor::White));
            if change_view {
                *app.world_mut().resource_mut::<ViewMode>() = ViewMode::Standard3D;
            } else {
                app.world_mut().resource_mut::<BoardEpoch>().bump();
            }
            app.update();
            assert!(!app.world().resource::<DragState2D>().dragging);
            assert!(app.world().resource::<DragState2D>().piece.is_none());
            *app.world_mut().resource_mut::<ViewMode>() = ViewMode::Standard2D;
            app.update();
        }
    }
}
