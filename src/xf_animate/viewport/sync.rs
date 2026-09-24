use bevy::camera::Viewport;
use bevy::prelude::*;

use super::camera::MiniShowcaseCamera;
use super::resource::LearnViewportRect;

pub fn sync_learn_viewport(
    viewport_rect: Res<LearnViewportRect>,
    windows: Query<&bevy::window::Window, With<bevy::window::PrimaryWindow>>,
    mut cameras: Query<&mut Camera, With<MiniShowcaseCamera>>,
) {
    let Ok(mut camera) = cameras.single_mut() else {
        return;
    };

    let window_size = windows
        .single()
        .ok()
        .map(|w| UVec2::new(w.physical_width(), w.physical_height()));

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
