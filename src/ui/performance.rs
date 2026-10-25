//! A small, read-only frame-rate overlay shared by every screen.

use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};

use super::styles::UiColors;

#[derive(Resource)]
struct PerformanceMonitor {
    visible: bool,
}

impl Default for PerformanceMonitor {
    fn default() -> Self {
        Self { visible: true }
    }
}

pub struct PerformanceMonitorPlugin;

impl Plugin for PerformanceMonitorPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(FrameTimeDiagnosticsPlugin::default())
            .init_resource::<PerformanceMonitor>()
            .add_systems(Update, toggle_monitor)
            .add_systems(EguiPrimaryContextPass, draw_monitor);
    }
}

fn toggle_monitor(keyboard: Res<ButtonInput<KeyCode>>, mut monitor: ResMut<PerformanceMonitor>) {
    if keyboard.just_pressed(KeyCode::F3) {
        monitor.visible = !monitor.visible;
    }
}

fn draw_monitor(
    mut contexts: EguiContexts,
    diagnostics: Res<DiagnosticsStore>,
    monitor: Res<PerformanceMonitor>,
) {
    if !monitor.visible {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    let frame_ms = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
        .and_then(|diagnostic| diagnostic.smoothed())
        .filter(|ms| ms.is_finite() && *ms > 0.0);
    // Derive both numbers from the same smoothed frame time so they agree.
    let fps = frame_ms.map(|ms| 1000.0 / ms);
    let color = match fps {
        Some(fps) if fps >= 60.0 => UiColors::SUCCESS,
        Some(fps) if fps >= 30.0 => UiColors::WARNING,
        Some(_) => UiColors::DANGER,
        None => UiColors::TEXT_SECONDARY,
    };

    egui::Area::new("performance_monitor".into())
        .order(egui::Order::Foreground)
        .movable(false)
        .interactable(false)
        .constrain_to(ctx.viewport_rect())
        // Keep the strip above the menu's wallet HUD, which starts at y=20.
        .anchor(egui::Align2::LEFT_TOP, [20.0, 0.0])
        .show(ctx, |ui| {
            egui::Frame::NONE
                .fill(egui::Color32::from_rgba_unmultiplied(20, 20, 25, 210))
                .corner_radius(3.0)
                .inner_margin(egui::Margin::symmetric(6, 0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(
                                fps.map_or_else(|| "-- FPS".into(), |fps| format!("{fps:3.0} FPS")),
                            )
                            .monospace()
                            .size(12.0)
                            .color(color),
                        );
                        ui.label(
                            egui::RichText::new(
                                frame_ms
                                    .map_or_else(|| "-- ms".into(), |ms| format!("{ms:4.1} ms")),
                            )
                            .monospace()
                            .size(12.0)
                            .color(UiColors::TEXT_PRIMARY),
                        );
                        ui.label(
                            egui::RichText::new("F3")
                                .monospace()
                                .size(10.0)
                                .color(UiColors::TEXT_SECONDARY),
                        );
                    });
                });
        });
}
