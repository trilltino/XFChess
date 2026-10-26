//! Scales every egui screen with the window.
//!
//! Panels, fonts and buttons are laid out in fixed points (a 260-point side panel,
//! 12-point text). At 1280x720 that reads well; on a 1080p or 1440p window it is tiny
//! and the board is dwarfed, in a small window it crowds the board. egui's zoom factor
//! multiplies the OS scale for the whole context, so one number resizes the menus, the
//! HUD, the 2D board and every popup together.
//!
//! The 3D board follows through [`crate::game::systems::camera::fit_board_camera`],
//! which reads the same [`UiZoom`] to know how wide the side panels are.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass};

/// The window size the layouts were designed at, in logical pixels.
const REFERENCE: Vec2 = Vec2::new(1280.0, 720.0);
const MIN_ZOOM: f32 = 0.8;
const MAX_ZOOM: f32 = 1.6;
/// Zoom moves in steps so dragging a window edge does not re-rasterise every glyph on
/// every pixel of travel.
const STEP: f32 = 0.05;

/// How much larger than designed the UI is drawn.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct UiZoom(pub f32);

impl Default for UiZoom {
    fn default() -> Self {
        Self(1.0)
    }
}

/// The zoom for a window of `width` x `height` logical pixels: the layout grows or
/// shrinks with whichever dimension is tighter, within sane bounds.
pub fn zoom_for(width: f32, height: f32) -> f32 {
    if !(width.is_finite() && height.is_finite()) || width <= 0.0 || height <= 0.0 {
        return 1.0;
    }
    let raw = (width / REFERENCE.x).min(height / REFERENCE.y);
    ((raw / STEP).round() * STEP).clamp(MIN_ZOOM, MAX_ZOOM)
}

fn track_window_size(windows: Query<&Window, With<PrimaryWindow>>, mut zoom: ResMut<UiZoom>) {
    let Ok(window) = windows.single() else {
        return;
    };
    let wanted = UiZoom(zoom_for(window.width(), window.height()));
    // Only a real change touches the resource, so nothing downstream re-runs per frame.
    if *zoom != wanted {
        *zoom = wanted;
    }
}

fn apply_zoom(mut contexts: EguiContexts, zoom: Res<UiZoom>) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    if (ctx.zoom_factor() - zoom.0).abs() > f32::EPSILON {
        ctx.set_zoom_factor(zoom.0);
    }
}

pub struct ResponsiveUiPlugin;

impl Plugin for ResponsiveUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiZoom>()
            .add_systems(Update, track_window_size)
            // The zoom takes effect on egui's next pass, so it is set before the screens draw.
            .add_systems(EguiPrimaryContextPass, apply_zoom);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_design_size_is_drawn_as_designed() {
        assert_eq!(zoom_for(1280.0, 720.0), 1.0);
    }

    #[test]
    fn a_larger_window_scales_the_ui_up_to_a_limit() {
        assert_eq!(zoom_for(1920.0, 1080.0), 1.5);
        assert_eq!(zoom_for(3840.0, 2160.0), MAX_ZOOM);
    }

    #[test]
    fn a_small_window_shrinks_the_ui_but_not_below_readable() {
        assert!(zoom_for(1024.0, 576.0) < 1.0);
        assert_eq!(zoom_for(600.0, 400.0), MIN_ZOOM);
    }

    #[test]
    fn the_tighter_dimension_decides() {
        // A tall, narrow window is limited by its width, a wide, short one by its height.
        assert_eq!(zoom_for(1000.0, 2000.0), zoom_for(1000.0, 720.0));
        assert_eq!(zoom_for(3000.0, 720.0), zoom_for(1280.0, 720.0));
    }

    #[test]
    fn dragging_a_window_edge_does_not_change_the_zoom_every_pixel() {
        assert_eq!(zoom_for(1500.0, 900.0), zoom_for(1503.0, 900.0));
    }

    #[test]
    fn a_degenerate_window_leaves_the_ui_alone() {
        assert_eq!(zoom_for(0.0, 0.0), 1.0);
        assert_eq!(zoom_for(f32::NAN, 720.0), 1.0);
    }
}
