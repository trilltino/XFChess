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
    /// Whether the user clicked the X to dismiss early.
    pub(crate) dismissed: bool,
    /// When this popup was created — used for fade-in / fade-out.
    pub(crate) created_at: std::time::Instant,
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
            created_at: std::time::Instant::now(),
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
/// Styled to match the "XFChess Alpha" welcome dialog with fade-in/fade-out.
fn render_popups(
    mut queue: ResMut<GamePopupQueue>,
    mut contexts: EguiContexts,
    mut ready: Local<u32>,
    _time: Res<Time>,
) {
    *ready += 1;
    if *ready < 3 || queue.entries.is_empty() {
        return;
    }

    let Ok(ctx) = contexts.ctx_mut() else { return };

    let margin = 16.0_f32;
    let width = 300.0_f32;
    let mut y_offset = margin;

    const FADE_IN_DUR: f32 = 0.3;
    const FADE_OUT_DUR: f32 = 1.5;

    let count = queue.entries.len();
    for i in (0..count).rev() {
        let popup = &queue.entries[i];

        // Fade alpha: fade in over FADE_IN_DUR, fade out over FADE_OUT_DUR
        let elapsed = popup.created_at.elapsed().as_secs_f32();
        let total = popup.lifetime;
        let fade_alpha = if elapsed < FADE_IN_DUR {
            (elapsed / FADE_IN_DUR).clamp(0.0, 1.0)
        } else if total.is_finite() && elapsed > total - FADE_OUT_DUR {
            ((total - elapsed) / FADE_OUT_DUR).clamp(0.0, 1.0)
        } else {
            1.0
        };

        if fade_alpha <= 0.001 && total.is_finite() {
            queue.entries[i].dismissed = true;
            continue;
        }

        // Alpha-dialog frame (same as welcome panel in new_menu.rs)
        let panel_frame = egui::Frame {
            corner_radius: egui::CornerRadius::same(8),
            fill: egui::Color32::from_rgba_unmultiplied(8, 8, 12, (240.0 * fade_alpha) as u8),
            stroke: egui::Stroke::new(
                1.0,
                egui::Color32::from_rgba_unmultiplied(255, 255, 255, (28.0 * fade_alpha) as u8),
            ),
            inner_margin: egui::Margin::symmetric(18, 16),
            ..egui::Frame::NONE
        };

        let body_color =
            egui::Color32::from_rgba_unmultiplied(210, 215, 225, (255.0 * fade_alpha) as u8);
        let accent_faded =
            egui::Color32::from_rgba_unmultiplied(100, 200, 255, (255.0 * fade_alpha) as u8);
        let close_color =
            egui::Color32::from_rgba_unmultiplied(180, 180, 180, (255.0 * fade_alpha) as u8);

        let mut open = true;
        let mut close_clicked = false;
        let title = popup.title.clone();
        let message = popup.message.clone();
        let copy_text = popup.copy_text.clone();
        let url = popup.url.clone();
        let url_label = popup
            .url_label
            .clone()
            .unwrap_or_else(|| "Open".to_string());

        let win_resp = egui::Window::new(&title)
            .id(egui::Id::new(("popup", &title)))
            .title_bar(false)
            .resizable(false)
            .collapsible(false)
            .movable(false)
            .anchor(egui::Align2::RIGHT_BOTTOM, [-margin, -(y_offset)])
            .fixed_size([width, 0.0])
            .frame(panel_frame)
            .show(ctx, |ui| {
                ui.set_opacity(fade_alpha);

                // Header: centered title + close button on the right
                let header_height = 24.0;
                let close_width = 24.0;
                let title_width = (ui.available_width() - close_width).max(0.0);
                ui.horizontal(|ui| {
                    // Title centered in the remaining width
                    ui.allocate_ui_with_layout(
                        egui::vec2(title_width, header_height),
                        egui::Layout::from_main_dir_and_cross_align(
                            egui::Direction::TopDown,
                            egui::Align::Center,
                        )
                        .with_main_align(egui::Align::Center),
                        |ui| {
                            ui.label(
                                egui::RichText::new(&title)
                                    .size(17.0)
                                    .color(accent_faded)
                                    .strong(),
                            );
                        },
                    );

                    // Close button
                    let close = ui.add_sized(
                        [close_width, header_height],
                        egui::Button::new(egui::RichText::new("X").size(13.0).color(close_color))
                            .fill(egui::Color32::TRANSPARENT)
                            .stroke(egui::Stroke::NONE),
                    );
                    if close.clicked() {
                        close_clicked = true;
                    }
                    if close.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                });

                ui.add_space(8.0);
                ui.add(egui::Separator::default().horizontal());
                ui.add_space(8.0);

                // Body
                ui.label(egui::RichText::new(&message).size(12.0).color(body_color));

                if let Some(ref ct) = copy_text {
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "{}\u{2026}{}",
                                &ct[..ct.len().min(6)],
                                &ct[ct.len().saturating_sub(4)..]
                            ))
                            .size(11.0)
                            .color(egui::Color32::from_rgba_unmultiplied(
                                160,
                                160,
                                160,
                                (255.0 * fade_alpha) as u8,
                            ))
                            .monospace(),
                        );
                        if ui
                            .small_button(egui::RichText::new("Copy").size(11.0))
                            .on_hover_text("Copy address")
                            .clicked()
                        {
                            ui.output_mut(|o| {
                                o.commands.push(egui::OutputCommand::CopyText(ct.clone()))
                            });
                        }
                    });
                }

                if let Some(ref u) = url {
                    ui.add_space(4.0);
                    if ui
                        .button(
                            egui::RichText::new(&url_label)
                                .size(12.0)
                                .color(accent_faded),
                        )
                        .clicked()
                    {
                        let url_to_open = u.clone();
                        if let Err(e) = webbrowser::open(&url_to_open) {
                            warn!("Failed to open popup link '{}': {}", url_to_open, e);
                        }
                    }
                }
            });

        if close_clicked {
            open = false;
        }

        if let Some(inner) = win_resp {
            y_offset += inner.response.rect.height() + 8.0;
        } else {
            y_offset += 120.0;
        }

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
