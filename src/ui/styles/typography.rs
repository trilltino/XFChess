//! Typography system for XFChess UI
//!
//! Defines consistent text sizes, styles, and formatting.

use super::colors::UiColors;
use bevy_egui::egui;

/// Text size presets
pub struct TextSize;

impl TextSize {
    /// Extra large heading (main menu title)
    pub const XL: f32 = 48.0;

    /// Large heading (section titles)
    pub const LG: f32 = 32.0;

    /// Medium heading (sub-sections)
    pub const MD: f32 = 24.0;

    /// Small heading (card titles)
    pub const SM: f32 = 18.0;

    /// Body text
    pub const BODY: f32 = 14.0;

    /// Small text (hints, captions)
    pub const CAPTION: f32 = 12.0;
}

/// Helper functions for creating styled text
#[allow(dead_code)] // Reserved for future UI styling
pub struct TextStyle;

impl TextStyle {
    /// Create a heading with the XFChess style
    #[allow(dead_code)] // Reserved for future UI styling
    pub fn heading(text: impl Into<String>, size: f32) -> egui::RichText {
        egui::RichText::new(text)
            .size(size)
            .color(UiColors::TEXT_PRIMARY)
            .strong()
    }

    /// Create body text
    pub fn body(text: impl Into<String>) -> egui::RichText {
        egui::RichText::new(text)
            .size(TextSize::BODY)
            .color(UiColors::TEXT_SECONDARY)
    }

    /// Create caption text (small, less prominent)
    pub fn caption(text: impl Into<String>) -> egui::RichText {
        egui::RichText::new(text)
            .size(TextSize::CAPTION)
            .color(UiColors::TEXT_TERTIARY)
    }

    /// Create accent text (gold color)
    #[allow(dead_code)] // Reserved for future UI styling
    pub fn accent(text: impl Into<String>) -> egui::RichText {
        egui::RichText::new(text)
            .size(TextSize::BODY)
            .color(UiColors::ACCENT_GOLD)
            .strong()
    }

    /// Create success text (green)
    #[allow(dead_code)] // Reserved for future UI styling
    pub fn success(text: impl Into<String>) -> egui::RichText {
        egui::RichText::new(text)
            .size(TextSize::BODY)
            .color(UiColors::SUCCESS)
    }

    /// Create warning text (orange)
    #[allow(dead_code)] // Reserved for future UI styling
    pub fn warning(text: impl Into<String>) -> egui::RichText {
        egui::RichText::new(text)
            .size(TextSize::BODY)
            .color(UiColors::WARNING)
    }

    /// Create error text (red)
    #[allow(dead_code)] // Reserved for future UI styling
    pub fn error(text: impl Into<String>) -> egui::RichText {
        egui::RichText::new(text)
            .size(TextSize::BODY)
            .color(UiColors::DANGER)
    }

    /// Create button text
    pub fn button(text: impl Into<String>, size: f32) -> egui::RichText {
        egui::RichText::new(text)
            .size(size)
            .color(UiColors::TEXT_PRIMARY)
    }
}
