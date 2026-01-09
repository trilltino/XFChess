use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

use crate::ui::styles::{TextStyle, UiColors};

/// System to display FPS counter
pub fn fps_ui(mut contexts: EguiContexts, diagnostics: Res<DiagnosticsStore>) {
    let ctx = match contexts.ctx_mut() {
        Ok(ctx) => ctx,
        Err(_) => return,
    };

    // Top-left floating FPS counter
    egui::Window::new("fps_counter")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::LEFT_TOP, [10.0, 10.0])
        .frame(
            egui::Frame::default()
                .fill(UiColors::BG_OVERLAY)
                .corner_radius(5.0)
                .inner_margin(5.0)
                .stroke(egui::Stroke::new(1.0, UiColors::BORDER)),
        )
        .show(ctx, |ui| {
            if let Some(fps) = diagnostics.get(&FrameTimeDiagnosticsPlugin::FPS) {
                if let Some(value) = fps.smoothed() {
                    let color = if value >= 55.0 {
                        UiColors::SUCCESS
                    } else if value >= 30.0 {
                        UiColors::WARNING
                    } else {
                        UiColors::DANGER
                    };

                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("{:.0}", value))
                                .size(14.0)
                                .color(color)
                                .strong(),
                        );
                        ui.label(
                            egui::RichText::new("FPS")
                                .size(10.0)
                                .color(UiColors::TEXT_SECONDARY),
                        );
                    });
                } else {
                    ui.label(
                        egui::RichText::new("FPS: --")
                            .size(10.0)
                            .color(UiColors::TEXT_TERTIARY),
                    );
                }
            }
        });
}
