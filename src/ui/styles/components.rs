//! Styled UI component builders
//!
//! Provides helper functions to create consistently styled UI components.

use super::colors::UiColors;
use bevy_egui::egui;

/// Vertically centers `add_contents` within the remaining space of `ui`.
///
/// Immediate-mode UI can't know a block's height before laying it out, so
/// this pads with the *previous frame's* measured height (via egui's
/// per-id temp storage) and re-measures after. Content settles into place
/// within a frame or two of first appearing and stays stable thereafter.
pub fn vertically_center<R>(
    ui: &mut egui::Ui,
    id: egui::Id,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let available = ui.available_height();
    let last_height = ui.ctx().data(|d| d.get_temp::<f32>(id)).unwrap_or(0.0);
    let top_pad = ((available - last_height) * 0.5).max(0.0);
    ui.add_space(top_pad);
    let inner = ui.vertical(add_contents);
    ui.ctx()
        .data_mut(|d| d.insert_temp(id, inner.response.rect.height()));
    inner.inner
}

/// Small non-interactive rounded pill badge (top-bar mode/time-control tags,
/// the left sidebar's "MATCH" label, etc.) — same shape as
/// [`StyledButton::chip`] minus the click sense, since these are
/// informational tags rather than actionable buttons.
pub fn pill(ui: &mut egui::Ui, text: &str, fill: egui::Color32, text_color: egui::Color32) {
    egui::Frame::default()
        .fill(fill)
        .corner_radius(4.0)
        .inner_margin(egui::Margin::symmetric(10, 4))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(text)
                    .size(11.0)
                    .strong()
                    .color(text_color),
            );
        });
}

/// A big bold value over a small uppercase caption — the left sidebar's
/// TIME CONTROL / OPPONENT RATING / YOUR RATING stat rows.
pub fn stat_row(ui: &mut egui::Ui, value: &str, caption: &str) {
    ui.vertical(|ui| {
        ui.label(
            egui::RichText::new(value)
                .size(22.0)
                .strong()
                .color(UiColors::TEXT_PRIMARY),
        );
        ui.label(
            egui::RichText::new(caption.to_uppercase())
                .size(10.5)
                .color(UiColors::TEXT_TERTIARY),
        );
    });
}

/// Two-way segmented toggle (e.g. "2D VIEW" / "3D VIEW" in the top bar).
/// Returns `true` if `right_label` should now be selected (i.e. the user
/// clicked the non-active side); returns `None` if neither side was clicked.
pub fn segmented_toggle(
    ui: &mut egui::Ui,
    left_label: &str,
    right_label: &str,
    right_selected: bool,
) -> Option<bool> {
    let mut clicked = None;
    egui::Frame::default()
        .fill(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 10))
        .corner_radius(6.0)
        .inner_margin(egui::Margin::same(2))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                if StyledButton::chip(ui, left_label, !right_selected, egui::Vec2::new(70.0, 26.0))
                    .clicked()
                    && right_selected
                {
                    clicked = Some(false);
                }
                if StyledButton::chip(ui, right_label, right_selected, egui::Vec2::new(70.0, 26.0))
                    .clicked()
                    && !right_selected
                {
                    clicked = Some(true);
                }
            });
        });
    clicked
}

/// Helper functions for spacing and layout
pub struct Layout;

impl Layout {
    /// Standard spacing between sections
    pub const SECTION_SPACING: f32 = 30.0;

    /// Standard spacing between items
    pub const ITEM_SPACING: f32 = 15.0;

    /// Small spacing
    pub const SMALL_SPACING: f32 = 8.0;

    /// Fixed width for the in-game left/right side panels. Both panels use
    /// this exact (non-resizable) width so the board column left between
    /// them is identical in size regardless of which side is measured —
    /// this is also what keeps the 2D and 3D board renders the same size.
    pub const SIDE_PANEL_WIDTH: f32 = 320.0;

    /// Add section spacing
    pub fn section_space(ui: &mut egui::Ui) {
        ui.add_space(Self::SECTION_SPACING);
    }

    /// Add item spacing
    pub fn item_space(ui: &mut egui::Ui) {
        ui.add_space(Self::ITEM_SPACING);
    }

    /// Add small spacing
    pub fn small_space(ui: &mut egui::Ui) {
        ui.add_space(Self::SMALL_SPACING);
    }
}

/// Modern glassmorphic buttons with transparent backgrounds
pub struct ModernButton;

impl ModernButton {
    /// Create a primary action button with glassmorphic effect
    pub fn primary(ui: &mut egui::Ui, text: &str) -> egui::Response {
        let button = egui::Button::new(text)
            .fill(egui::Color32::from_rgba_premultiplied(40, 40, 45, 200))
            .stroke(egui::Stroke::new(
                1.0,
                egui::Color32::from_rgb(218, 165, 32),
            ));
        ui.add(button)
    }

    /// Create a secondary action button with glassmorphic effect
    pub fn secondary(ui: &mut egui::Ui, text: &str) -> egui::Response {
        let button = egui::Button::new(text)
            .fill(egui::Color32::from_rgba_premultiplied(30, 30, 35, 180))
            .stroke(egui::Stroke::new(
                1.0,
                egui::Color32::from_rgb(100, 100, 105),
            ));
        ui.add(button)
    }
}

/// Helper functions for creating styled buttons
pub struct StyledButton;

impl StyledButton {
    /// Create a primary action button (gold accent)
    pub fn primary(ui: &mut egui::Ui, text: &str) -> egui::Response {
        let button = egui::Button::new(text)
            .fill(UiColors::ACCENT_GOLD)
            .stroke(egui::Stroke::new(1.0, UiColors::BORDER));
        ui.add(button)
    }

    /// Create a small button for less important actions
    pub fn small(ui: &mut egui::Ui, text: &str) -> egui::Response {
        let button = egui::Button::new(text)
            .fill(UiColors::BG_LIGHT)
            .stroke(egui::Stroke::new(1.0, UiColors::BORDER));
        ui.add(button)
    }

    /// Create a danger button (red, for destructive actions)
    pub fn danger(ui: &mut egui::Ui, text: &str) -> egui::Response {
        let button = egui::Button::new(text)
            .fill(UiColors::DANGER)
            .stroke(egui::Stroke::new(1.0, UiColors::BORDER));
        ui.add(button)
    }

    /// Create a secondary action button
    pub fn secondary(ui: &mut egui::Ui, text: &str) -> egui::Response {
        let button = egui::Button::new(text)
            .fill(UiColors::BG_LIGHT)
            .stroke(egui::Stroke::new(1.0, UiColors::BORDER));
        ui.add(button)
    }

    /// Full-width (or fixed-width) outlined pill button — transparent fill,
    /// colored stroke + text. Used for the in-game Resign / Offer Draw
    /// controls in the right sidebar, which read as bordered pills rather
    /// than filled buttons or borderless text like every other variant here.
    pub fn outline_pill(
        ui: &mut egui::Ui,
        text: &str,
        color: egui::Color32,
        full_width: bool,
        enabled: bool,
    ) -> egui::Response {
        let width = if full_width {
            ui.available_width()
        } else {
            140.0
        };
        let shown_color = if enabled {
            color
        } else {
            color.gamma_multiply(0.45)
        };
        let button = egui::Button::new(
            egui::RichText::new(text)
                .size(13.0)
                .strong()
                .color(shown_color),
        )
        .fill(egui::Color32::TRANSPARENT)
        .stroke(egui::Stroke::new(1.0, shown_color))
        .corner_radius(6.0)
        .min_size(egui::Vec2::new(width, 36.0));
        ui.add_enabled(enabled, button)
    }

    /// Toggle-style "chip" button for option rows (strength, time control,
    /// engine, color/side, filters, ...). One shared look for every such
    /// row across every popup, instead of each screen hand-rolling its own
    /// fill/stroke/radius combination.
    ///
    /// `min_size` lets callers keep per-row sizing (e.g. wider engine-name
    /// chips vs. square strength-number chips) while sharing colors/shape.
    pub fn chip(
        ui: &mut egui::Ui,
        text: &str,
        selected: bool,
        min_size: egui::Vec2,
    ) -> egui::Response {
        let accent = UiColors::ACCENT;
        let button = egui::Button::new(egui::RichText::new(text).size(13.5).color(if selected {
            egui::Color32::WHITE
        } else {
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 160)
        }))
        .min_size(min_size)
        .corner_radius(6.0)
        .fill(if selected {
            accent
        } else {
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 8)
        })
        .stroke(egui::Stroke::new(
            1.0,
            if selected {
                accent
            } else {
                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 20)
            },
        ));
        ui.add(button)
    }
}

/// Helper functions for creating styled panels
pub struct StyledPanel;

impl StyledPanel {
    /// Create a main content panel
    pub fn main() -> egui::Frame {
        egui::Frame {
            fill: UiColors::BG_DARK,
            stroke: egui::Stroke::new(1.0, UiColors::BORDER),
            corner_radius: egui::CornerRadius::same(8),
            shadow: egui::epaint::Shadow::NONE,
            inner_margin: egui::Margin::same(16),
            outer_margin: egui::Margin::ZERO,
        }
    }

    /// Create a card panel
    pub fn card() -> egui::Frame {
        egui::Frame {
            fill: UiColors::BG_MID,
            stroke: egui::Stroke::new(1.0, UiColors::BORDER),
            corner_radius: egui::CornerRadius::same(8),
            shadow: egui::epaint::Shadow::NONE,
            inner_margin: egui::Margin::same(12),
            outer_margin: egui::Margin::ZERO,
        }
    }

    /// Create an overlay panel
    pub fn overlay() -> egui::Frame {
        egui::Frame {
            fill: UiColors::BG_OVERLAY,
            stroke: egui::Stroke::NONE,
            corner_radius: egui::CornerRadius::same(8),
            shadow: egui::epaint::Shadow::NONE,
            inner_margin: egui::Margin::ZERO,
            outer_margin: egui::Margin::ZERO,
        }
    }

    /// Standard modal/popup frame — the single look shared by every popup in
    /// the app (menu setup modals, lobby/tournament/host-config screens,
    /// in-game promotion/resign/draw/game-over dialogs), so no popup reads as
    /// visually distinct from another.
    pub fn popup() -> egui::Frame {
        egui::Frame {
            fill: egui::Color32::from_rgba_unmultiplied(18, 18, 22, 242),
            inner_margin: egui::Margin::same(20),
            outer_margin: egui::Margin::ZERO,
            corner_radius: egui::CornerRadius::same(8),
            stroke: egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(80, 80, 100, 180)),
            shadow: egui::Shadow {
                blur: 24,
                spread: 4,
                color: egui::Color32::from_black_alpha(180),
                offset: [0, 4],
            },
        }
    }

    /// Popup frame with variable alpha for fade-in animations.
    pub fn popup_alpha(alpha: u8) -> egui::Frame {
        egui::Frame {
            fill: egui::Color32::from_rgba_unmultiplied(8, 10, 18, alpha),
            corner_radius: egui::CornerRadius::same(14),
            stroke: egui::Stroke::NONE,
            inner_margin: egui::Margin::same(24),
            ..egui::Frame::NONE
        }
    }

    /// Flat, borderless card used for sub-sections inside the in-game left
    /// and right sidebars (player row, clock, move list, controls, chat).
    /// No stroke — a lichess-style sidebar reads as one flat surface with
    /// subtle fill differences between sections, not a stack of boxed panels.
    pub fn sidebar_card() -> egui::Frame {
        egui::Frame {
            fill: egui::Color32::from_rgba_unmultiplied(0, 0, 0, 35),
            stroke: egui::Stroke::NONE,
            corner_radius: egui::CornerRadius::same(6),
            shadow: egui::epaint::Shadow::NONE,
            inner_margin: egui::Margin::symmetric(12, 8),
            outer_margin: egui::Margin::ZERO,
        }
    }

    /// Transparent variant of [`Self::sidebar_card`] — same margins/radius
    /// but no fill, for rows that shouldn't stand out from the panel behind
    /// them (e.g. name rows sitting directly above a filled clock card).
    pub fn sidebar_row() -> egui::Frame {
        egui::Frame {
            fill: egui::Color32::TRANSPARENT,
            stroke: egui::Stroke::NONE,
            corner_radius: egui::CornerRadius::ZERO,
            shadow: egui::epaint::Shadow::NONE,
            inner_margin: egui::Margin::symmetric(12, 6),
            outer_margin: egui::Margin::ZERO,
        }
    }
}
