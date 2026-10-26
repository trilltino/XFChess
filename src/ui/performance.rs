//! A small, read-only frame-rate overlay shared by every screen.

use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};

use super::styles::UiColors;

/// How often the numbers are redrawn. Redrawing every frame makes the digits flicker
/// even when the frame rate is steady, and nobody can read a value that changes 60 times
/// a second.
const REFRESH_SECS: f64 = 0.5;

#[derive(Clone, Copy)]
struct Reading {
    fps: f64,
    frame_ms: f64,
}

#[derive(Resource)]
struct PerformanceMonitor {
    visible: bool,
    shown: Option<Reading>,
    shown_at: f64,
}

impl Default for PerformanceMonitor {
    fn default() -> Self {
        Self {
            visible: true,
            shown: None,
            shown_at: f64::NEG_INFINITY,
        }
    }
}

/// Whether the displayed reading is old enough to replace.
fn refresh_due(shown_at: f64, now: f64) -> bool {
    now - shown_at >= REFRESH_SECS
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
    time: Res<Time<Real>>,
    mut monitor: ResMut<PerformanceMonitor>,
) {
    if !monitor.visible {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    let now = time.elapsed_secs_f64();
    if monitor.shown.is_none() || refresh_due(monitor.shown_at, now) {
        // The mean over the diagnostic history (about two seconds), not the latest frame.
        let measured = diagnostics
            .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
            .and_then(|diagnostic| diagnostic.average())
            .filter(|ms| ms.is_finite() && *ms > 0.0);
        if let Some(frame_ms) = measured {
            // Both numbers come from the same frame time so they agree.
            monitor.shown = Some(Reading {
                fps: 1000.0 / frame_ms,
                frame_ms,
            });
            monitor.shown_at = now;
        }
    }
    let frame_ms = monitor.shown.map(|reading| reading.frame_ms);
    let fps = monitor.shown.map(|reading| reading.fps);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_reading_holds_between_refreshes() {
        assert!(!refresh_due(10.0, 10.2));
        assert!(refresh_due(10.0, 10.5));
        assert!(
            refresh_due(f64::NEG_INFINITY, 0.0),
            "the first reading is not held back"
        );
    }
}
