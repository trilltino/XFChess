//! Typography system for XFChess UI
//!
//! Defines consistent text sizes, styles, and formatting.

use super::colors::UiColors;
use bevy_egui::egui;

/// Helper functions for creating styled text
#[allow(dead_code)]
pub struct TextStyle;

impl TextStyle {
    /// Create a heading with the XFChess style
    #[allow(dead_code)]
    pub fn heading(text: impl Into<String>, size: f32) -> egui::RichText {
        egui::RichText::new(text)
            .size(size)
            .color(UiColors::TEXT_PRIMARY)
            .strong()
    }

    /// Create body text
    #[allow(dead_code)]
    pub fn body(text: impl Into<String>) -> egui::RichText {
        egui::RichText::new(text)
            .size(TextSize::BODY)
            .color(UiColors::TEXT_SECONDARY)
    }

    /// Create caption text (small, less prominent)
    #[allow(dead_code)]
    pub fn caption(text: impl Into<String>) -> egui::RichText {
        egui::RichText::new(text)
            .size(TextSize::CAPTION)
            .color(UiColors::TEXT_TERTIARY)
    }
}

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
    #[allow(dead_code)]
    pub const SM: f32 = 18.0;

    /// Body text
    pub const BODY: f32 = 14.0;

    /// Small text (hints, captions)
    pub const CAPTION: f32 = 12.0;
}

