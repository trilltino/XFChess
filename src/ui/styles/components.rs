//! Styled UI component builders
//!
//! Provides helper functions to create consistently styled UI components.

use super::colors::UiColors;
use super::typography::{TextSize, TextStyle};
use bevy_egui::egui;

/// Modern glassmorphic buttons with transparent backgrounds
#[allow(dead_code)] // Reserved for future UI styling
pub struct ModernButton;

impl ModernButton {
    /// Create a primary action button with glassmorphic effect
    #[allow(dead_code)] // Reserved for future UI styling
    pub fn primary(ui: &mut egui::Ui, text: impl Into<String>) -> egui::Response {
        let text_str = text.into();
        let desired_size = egui::vec2(320.0, 56.0);

        // Create unique ID from text
        let button_id = ui.make_persistent_id(&text_str);
        let (rect, _) = ui.allocate_at_least(desired_size, egui::Sense::click());

        // Get interaction response with unique ID
        let response = ui.interact(rect, button_id, egui::Sense::click());

        // Determine visual state from response
        let is_hovered = response.hovered();
        let is_pressed = response.is_pointer_button_down_on();

        // Choose colors based on state
        let (fill_color, stroke_color, text_color, stroke_width) = if is_pressed {
            (
                egui::Color32::from_rgba_premultiplied(255, 255, 255, 90),
                egui::Color32::from_rgba_premultiplied(255, 255, 255, 200),
                egui::Color32::from_rgba_premultiplied(255, 255, 255, 255),
                2.5,
            )
        } else if is_hovered {
            (
                egui::Color32::from_rgba_premultiplied(255, 255, 255, 70),
                egui::Color32::from_rgba_premultiplied(255, 255, 255, 180),
                egui::Color32::from_rgba_premultiplied(255, 255, 255, 255),
                2.0,
            )
        } else {
            (
                egui::Color32::from_rgba_premultiplied(255, 255, 255, 40),
                egui::Color32::from_rgba_premultiplied(255, 255, 255, 120),
                egui::Color32::from_rgba_premultiplied(255, 255, 255, 250),
                1.5,
            )
        };

        // Draw button background and border
        let corner_radius = egui::CornerRadius::same(12);
        ui.painter().rect_filled(rect, corner_radius, fill_color);
        ui.painter().rect_stroke(
            rect,
            corner_radius,
            egui::Stroke::new(stroke_width, stroke_color),
            egui::StrokeKind::Inside,
        );

        // Draw text
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            &text_str,
            egui::FontId::proportional(18.0),
            text_color,
        );

        response
    }

    /// Create a secondary action button with glassmorphic effect
    #[allow(dead_code)] // Reserved for future UI styling
    pub fn secondary(ui: &mut egui::Ui, text: impl Into<String>) -> egui::Response {
        let text_str = text.into();
        let desired_size = egui::vec2(200.0, 48.0);

        // Create unique ID from text
        let button_id = ui.make_persistent_id(&text_str);
        let (rect, _) = ui.allocate_at_least(desired_size, egui::Sense::click());

        // Get interaction response with unique ID
        let response = ui.interact(rect, button_id, egui::Sense::click());

        // Determine visual state from response
        let is_hovered = response.hovered();
        let is_pressed = response.is_pointer_button_down_on();

        // Choose colors based on state
        let (fill_color, stroke_color, text_color, stroke_width) = if is_pressed {
            (
                egui::Color32::from_rgba_premultiplied(0, 0, 0, 80),
                egui::Color32::from_rgba_premultiplied(255, 255, 255, 140),
                egui::Color32::from_rgba_premultiplied(255, 255, 255, 255),
                2.0,
            )
        } else if is_hovered {
            (
                egui::Color32::from_rgba_premultiplied(0, 0, 0, 60),
                egui::Color32::from_rgba_premultiplied(255, 255, 255, 120),
                egui::Color32::from_rgba_premultiplied(255, 255, 255, 255),
                1.5,
            )
        } else {
            (
                egui::Color32::from_rgba_premultiplied(0, 0, 0, 30),
                egui::Color32::from_rgba_premultiplied(255, 255, 255, 80),
                egui::Color32::from_rgba_premultiplied(255, 255, 255, 220),
                1.0,
            )
        };

        // Draw button background and border
        let corner_radius = egui::CornerRadius::same(10);
        ui.painter().rect_filled(rect, corner_radius, fill_color);
        ui.painter().rect_stroke(
            rect,
            corner_radius,
            egui::Stroke::new(stroke_width, stroke_color),
            egui::StrokeKind::Inside,
        );

        // Draw text
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            &text_str,
            egui::FontId::proportional(16.0),
            text_color,
        );

        response
    }
}

/// Helper functions for creating styled buttons
#[allow(dead_code)] // Reserved for future UI styling
pub struct StyledButton;

impl StyledButton {
    /// Create a primary action button (gold accent)
    #[allow(dead_code)] // Reserved for future UI styling
    pub fn primary(ui: &mut egui::Ui, text: impl Into<String>) -> egui::Response {
        let button = egui::Button::new(
            egui::RichText::new(text.into())
                .size(20.0)
                .color(egui::Color32::from_rgb(0, 0, 0)), // Black text
        )
        .fill(egui::Color32::from_rgb(255, 215, 0)) // Bright gold
        .stroke(egui::Stroke::new(
            2.0,
            egui::Color32::from_rgb(255, 255, 255),
        ))
        .min_size(egui::vec2(300.0, 60.0));

        ui.add(button)
    }

    /// Create a secondary action button
    pub fn secondary(ui: &mut egui::Ui, text: impl Into<String>) -> egui::Response {
        let button = egui::Button::new(TextStyle::button(text, TextSize::SM))
            .fill(UiColors::BG_LIGHT)
            .stroke(egui::Stroke::new(1.0, UiColors::BORDER))
            .min_size(egui::vec2(180.0, 40.0));

        ui.add(button)
    }

    /// Create a small button for less important actions
    #[allow(dead_code)] // Reserved for future UI styling
    pub fn small(ui: &mut egui::Ui, text: impl Into<String>) -> egui::Response {
        let button = egui::Button::new(TextStyle::button(text, TextSize::BODY))
            .fill(UiColors::BG_MID)
            .stroke(egui::Stroke::new(1.0, UiColors::BORDER))
            .min_size(egui::vec2(120.0, 35.0));

        ui.add(button)
    }

    /// Create a danger button (red, for destructive actions)
    pub fn danger(ui: &mut egui::Ui, text: impl Into<String>) -> egui::Response {
        let button = egui::Button::new(TextStyle::button(text, TextSize::SM))
            .fill(UiColors::DANGER)
            .stroke(egui::Stroke::NONE)
            .min_size(egui::vec2(150.0, 40.0));

        ui.add(button)
    }
}

/// Helper functions for creating styled panels
#[allow(dead_code)] // Reserved for future UI styling
pub struct StyledPanel;

impl StyledPanel {
    /// Create a main content panel
    #[allow(dead_code)] // Reserved for future UI styling
    pub fn main() -> egui::Frame {
        egui::Frame {
            fill: UiColors::BG_DARK,
            stroke: egui::Stroke::new(2.0, UiColors::BORDER),
            inner_margin: egui::Margin::same(20),
            outer_margin: egui::Margin::same(10),
            shadow: egui::epaint::Shadow {
                offset: [0, 4],
                blur: 12,
                spread: 0,
                color: egui::Color32::from_black_alpha(100),
            },
            ..Default::default()
        }
    }

    /// Create a card-style panel (for nested content)
    pub fn card() -> egui::Frame {
        egui::Frame {
            fill: UiColors::BG_MID,
            stroke: egui::Stroke::new(1.0, UiColors::BORDER),
            inner_margin: egui::Margin::same(15),
            outer_margin: egui::Margin::same(5),
            shadow: egui::epaint::Shadow {
                offset: [0, 2],
                blur: 6,
                spread: 0,
                color: egui::Color32::from_black_alpha(60),
            },
            ..Default::default()
        }
    }

    /// Create an overlay panel (semi-transparent, for modals)
    pub fn overlay() -> egui::Frame {
        egui::Frame {
            fill: UiColors::BG_OVERLAY,
            stroke: egui::Stroke::NONE,
            inner_margin: egui::Margin::same(30),
            outer_margin: egui::Margin::ZERO,
            shadow: egui::epaint::Shadow {
                offset: [0, 8],
                blur: 24,
                spread: 0,
                color: egui::Color32::from_black_alpha(150),
            },
            ..Default::default()
        }
    }
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
