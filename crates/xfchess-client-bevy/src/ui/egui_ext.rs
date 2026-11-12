//! egui helpers shared by every screen.
//!
//! Since egui 0.35, panels (`Panel::top/left/…`, `CentralPanel`) are laid out
//! inside a `Ui` rather than on the `Context`. Each screen builds one full-viewport
//! root with [`viewport_ui`] and shows its panels inside it.

use bevy_egui::egui;

/// A background-layer `Ui` covering the whole viewport, to show panels in.
pub fn viewport_ui(ctx: &egui::Context, id: &'static str) -> egui::Ui {
    egui::Ui::new(
        ctx.clone(),
        id.into(),
        egui::UiBuilder::new()
            .layer_id(egui::LayerId::background())
            .max_rect(ctx.viewport_rect()),
    )
}
