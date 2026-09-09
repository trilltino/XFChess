use super::colors::UiColors;
use bevy_egui::egui;

pub struct TextStyle;

impl TextStyle {
    pub fn heading(text: impl Into<String>, size: f32) -> egui::RichText {
        egui::RichText::new(text)
            .size(size)
            .color(UiColors::TEXT_PRIMARY)
            .strong()
    }

    pub fn body(text: impl Into<String>) -> egui::RichText {
        egui::RichText::new(text)
            .size(TextSize::BODY)
            .color(UiColors::TEXT_SECONDARY)
    }

    pub fn caption(text: impl Into<String>) -> egui::RichText {
        egui::RichText::new(text)
            .size(TextSize::CAPTION)
            .color(UiColors::TEXT_TERTIARY)
    }

    pub fn popup_title(text: impl Into<String>) -> egui::RichText {
        egui::RichText::new(text)
            .size(20.0)
            .color(egui::Color32::WHITE)
            .family(egui::FontFamily::Name("CinzelBold".into()))
    }

    pub fn popup_body(text: impl Into<String>) -> egui::RichText {
        egui::RichText::new(text)
            .size(13.0)
            .color(UiColors::TEXT_POPUP_BODY)
    }
}

pub struct TextSize;

impl TextSize {
    pub const XL: f32 = 48.0;

    pub const LG: f32 = 32.0;

    pub const MD: f32 = 24.0;

    pub const SM: f32 = 18.0;

    pub const BODY: f32 = 14.0;

    pub const CAPTION: f32 = 12.0;
}
