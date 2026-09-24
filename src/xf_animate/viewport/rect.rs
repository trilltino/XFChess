use bevy::prelude::*;
use bevy_egui::egui;

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
