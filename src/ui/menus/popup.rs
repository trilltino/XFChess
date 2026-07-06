//! In-game notification / popup system.
//!
//! Add a `GamePopup` to the `GamePopupQueue` resource from any system and it
//! will be rendered as a floating toast in the bottom-right corner while the
//! player is in-game. Popups auto-dismiss after a configurable timeout and
//! can carry an optional copy-to-clipboard string and an optional URL button.

use bevy::prelude::*;
use bevy_egui::egui;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass};

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// A single popup entry.
#[derive(Debug, Clone)]
pub struct GamePopup {
    pub title: String,
    pub message: String,
    /// Text that the "Copy" button will put on the clipboard (e.g. a pubkey).
    pub copy_text: Option<String>,
    /// URL opened in the system browser when the "Open" button is clicked.
    pub url: Option<String>,
    /// Display label for the URL button (defaults to "Open").
    pub url_label: Option<String>,
    /// How many seconds the popup stays visible before auto-dismissing.
    /// Set to `f32::INFINITY` to keep it until manually dismissed.
    pub lifetime: f32,
    /// Remaining time (decremented each frame; private — set by the queue).
    pub(crate) remaining: f32,
    /// Whether the user clicked the  to dismiss early.
    pub(crate) dismissed: bool,
}

impl GamePopup {
    pub fn warning(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            copy_text: None,
            url: None,
            url_label: None,
            lifetime: 12.0,
            remaining: 12.0,
            dismissed: false,
        }
    }

    pub fn with_copy(mut self, text: impl Into<String>) -> Self {
        self.copy_text = Some(text.into());
        self
    }

    pub fn with_url(mut self, url: impl Into<String>, label: impl Into<String>) -> Self {
        let url_str = url.into();
        let label_str = label.into();
        self.url = Some(url_str);
        self.url_label = Some(label_str);
        self
    }

    pub fn persistent(mut self) -> Self {
        self.lifetime = f32::INFINITY;
        self.remaining = f32::INFINITY;
        self
    }
}

// ---------------------------------------------------------------------------
// Queue resource
// ---------------------------------------------------------------------------

/// Global queue — push popups here from any system.
#[derive(Resource, Default)]
pub struct GamePopupQueue {
    pub entries: Vec<GamePopup>,
}

impl GamePopupQueue {
    pub fn push(&mut self, popup: GamePopup) {
        self.entries.push(popup);
    }
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// Tick lifetimes and remove expired / dismissed popups.
fn tick_popups(mut queue: ResMut<GamePopupQueue>, time: Res<Time>) {
    let dt = time.delta_secs();
    for p in &mut queue.entries {
        if p.remaining.is_finite() {
            p.remaining -= dt;
        }
    }
    queue.entries.retain(|p| !p.dismissed && p.remaining > 0.0);
}

/// Render all active popups as egui windows stacked bottom-right.
fn render_popups(
    mut queue: ResMut<GamePopupQueue>,
    mut contexts: EguiContexts,
    mut ready: Local<u32>,
) {
    // Skip first 2 frames: egui pass may not be initialized yet on frame 0-1
    *ready += 1;
    if *ready < 3 || queue.entries.is_empty() {
        return;
    }

    let Ok(ctx) = contexts.ctx_mut() else { return };

    let margin = 16.0_f32;
    let width = 300.0_f32;
    let mut y_offset = margin;

    // Render newest on top (iterate in reverse so we compute offsets bottom-up)
    let count = queue.entries.len();
    for i in (0..count).rev() {
        let popup = &queue.entries[i];

        let accent = egui::Color32::from_rgb(244, 187, 68); // gold

        let mut open = true;
        let title = popup.title.clone();
        let message = popup.message.clone();
        let copy_text = popup.copy_text.clone();
        let url = popup.url.clone();
        let url_label = popup
            .url_label
            .clone()
            .unwrap_or_else(|| "Open".to_string());
        let remaining = popup.remaining;
        let lifetime = popup.lifetime;

        let win_resp = egui::Window::new(&title)
            .id(egui::Id::new(("popup", &title))) // Use stable ID based on title
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .anchor(egui::Align2::RIGHT_BOTTOM, [-margin, -(y_offset)])
            .fixed_size([width, 0.0])
            .frame(
                egui::Frame::default()
                    .fill(egui::Color32::from_rgba_unmultiplied(10, 33, 26, 230))
                    .stroke(egui::Stroke::new(1.0, accent))
                    .corner_radius(12.0)
                    .inner_margin(14.0),
            )
            .show(ctx, |ui| {
                // Accent bar at top
                let (bar_rect, _) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), 3.0),
                    egui::Sense::hover(),
                );
                ui.painter().rect_filled(bar_rect, 2.0, accent);
                ui.add_space(8.0);

                ui.label(
                    egui::RichText::new(&message)
                        .size(12.5)
                        .color(egui::Color32::from_rgb(200, 200, 200)),
                );

                // Copy button
                if let Some(ref ct) = copy_text {
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "{}…{}",
                                &ct[..ct.len().min(6)],
                                &ct[ct.len().saturating_sub(4)..]
                            ))
                            .size(11.0)
                            .color(egui::Color32::from_rgb(160, 160, 160))
                            .monospace(),
                        );
                        if ui
                            .small_button(egui::RichText::new(" Copy").size(11.0))
                            .on_hover_text("Copy address")
                            .clicked()
                        {
                            ui.output_mut(|o| {
                                o.commands.push(egui::OutputCommand::CopyText(ct.clone()))
                            });
                        }
                    });
                }

                // URL button
                if let Some(ref u) = url {
                    ui.add_space(4.0);
                    if ui
                        .button(egui::RichText::new(&url_label).size(12.0).color(accent))
                        .clicked()
                    {
                        let url_to_open = u.clone();
                        if let Err(e) = webbrowser::open(&url_to_open) {
                            warn!("Failed to open popup link '{}': {}", url_to_open, e);
                        }
                    }
                }

                // Progress bar for timed popups
                if lifetime.is_finite() {
                    ui.add_space(8.0);
                    let progress = (remaining / lifetime).clamp(0.0, 1.0);
                    let (pb_rect, _) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), 3.0),
                        egui::Sense::hover(),
                    );
                    ui.painter().rect_filled(
                        pb_rect,
                        1.0,
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 25),
                    );
                    let mut filled = pb_rect;
                    filled.max.x = pb_rect.min.x + pb_rect.width() * progress;
                    ui.painter().rect_filled(filled, 1.0, accent);
                }
            });

        // Accumulate vertical space for next popup
        if let Some(inner) = win_resp {
            y_offset += inner.response.rect.height() + 8.0;
        } else {
            y_offset += 120.0;
        }

        // If the  button was clicked, mark dismissed
        if !open {
            queue.entries[i].dismissed = true;
        }
    }
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct PopupPlugin;

impl Plugin for PopupPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GamePopupQueue>()
            .add_systems(Update, tick_popups)
            .add_systems(EguiPrimaryContextPass, render_popups);
    }
}
