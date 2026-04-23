//! LEARN-box viewport bridge.
//!
//! Holds the resource written by the egui `render_learn_section` every frame
//! and syncs the mini-showcase camera's viewport to that rect.

use bevy::camera::visibility::RenderLayers;
use bevy::camera::{ClearColorConfig, Viewport};
use bevy::prelude::*;
use bevy_egui::egui;

use crate::core::{DespawnOnExit, GameState};

/// Render layer used exclusively by the mini-showcase scene and its camera.
pub const MINI_LAYER: usize = 8;

/// Physical-pixel rectangle allocated by egui for the LEARN-box viewport.
/// `None` means the viewport is hidden (e.g. loading screen, off-screen menu).
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct LearnViewportRect {
    pub rect_px: Option<URect>,
}

/// Marker for the secondary 3D camera rendering the mini showcase.
#[derive(Component)]
pub struct MiniShowcaseCamera;

/// Convert an egui rect (in egui points) to a physical pixel `URect`.
pub fn egui_rect_to_pixels(rect: egui::Rect, pixels_per_point: f32) -> URect {
    let ppp = pixels_per_point.max(0.0001);
    let min_x = (rect.min.x * ppp).max(0.0) as u32;
    let min_y = (rect.min.y * ppp).max(0.0) as u32;
    let max_x = (rect.max.x * ppp).max(0.0) as u32;
    let max_y = (rect.max.y * ppp).max(0.0) as u32;
    URect {
        min: UVec2::new(min_x, min_y),
        max: UVec2::new(max_x.max(min_x + 1), max_y.max(min_y + 1)),
    }
}

/// Spawn the secondary camera that renders only the mini showcase layer.
/// Positioned for a slight top-down angle over the 8x8 board centred at origin.
pub fn spawn_mini_camera(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Camera {
            order: 1,
            is_active: false,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        Projection::Perspective(PerspectiveProjection {
            fov: 0.9,
            ..default()
        }),
        Transform::from_xyz(0.0, 9.0, 9.0).looking_at(Vec3::ZERO, Vec3::Y),
        RenderLayers::layer(MINI_LAYER),
        MiniShowcaseCamera,
        DespawnOnExit(GameState::MainMenu),
        Name::new("XFAnimate Mini Camera"),
    ));
}

/// Pull the rect written by the egui LEARN section and apply it to the camera.
/// The rect is clamped to the primary window's physical size so wgpu never
/// receives an out-of-bounds scissor rectangle.
pub fn sync_learn_viewport(
    viewport_rect: Res<LearnViewportRect>,
    windows: Query<&bevy::window::Window, With<bevy::window::PrimaryWindow>>,
    mut cameras: Query<&mut Camera, With<MiniShowcaseCamera>>,
) {
    let Ok(mut camera) = cameras.single_mut() else {
        return;
    };

    let window_size = windows.single().ok().map(|w| {
        UVec2::new(w.physical_width(), w.physical_height())
    });

    let clamped = match (viewport_rect.rect_px, window_size) {
        (Some(rect), Some(ws)) if ws.x > 0 && ws.y > 0 => {
            let min = rect.min.min(ws);
            let max = rect.max.min(ws);
            if min.x + 2 >= max.x || min.y + 2 >= max.y {
                None
            } else {
                Some(URect { min, max })
            }
        }
        _ => None,
    };

    match clamped {
        Some(rect) => {
            camera.is_active = true;
            camera.viewport = Some(Viewport {
                physical_position: rect.min,
                physical_size: UVec2::new(rect.width(), rect.height()),
                depth: 0.0..1.0,
            });
        }
        None => {
            camera.is_active = false;
            camera.viewport = None;
        }
    }
}
