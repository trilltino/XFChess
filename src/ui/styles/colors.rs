//! Color palette for XFChess UI
//!
//! Defines a consistent color scheme inspired by chess aesthetics:
//! - Dark backgrounds (like a chess board)
//! - Gold/bronze accents (like tournament trophies)
//! - Clean text colors for readability
//!
//! Colors are defined as egui::Color32 for direct use in UI code.

use bevy_egui::egui;

/// Primary UI color palette
pub struct UiColors;

impl UiColors {
    // === Background Colors ===

    /// Primary dark background (main panels)
    pub const BG_DARK: egui::Color32 = egui::Color32::from_rgb(20, 20, 25);

    /// Secondary background (nested panels)
    pub const BG_MID: egui::Color32 = egui::Color32::from_rgb(30, 30, 35);

    /// Tertiary background (buttons, cards)
    pub const BG_LIGHT: egui::Color32 = egui::Color32::from_rgb(40, 40, 45);

    /// Overlay background (semi-transparent)
    pub const BG_OVERLAY: egui::Color32 = egui::Color32::from_black_alpha(150);

    // === Accent Colors ===

    /// Primary accent (gold - for important buttons and highlights)
    pub const ACCENT_GOLD: egui::Color32 = egui::Color32::from_rgb(218, 165, 32);

    /// Burnt-orange accent used for selected option "chips" (strength, time
    /// control, engine, side/color, filters) across every setup popup.
    pub const ACCENT: egui::Color32 = egui::Color32::from_rgb(173, 92, 47); // #ad5c2f

    /// Error/danger color (red)
    pub const DANGER: egui::Color32 = egui::Color32::from_rgb(220, 50, 50);

    /// Warning color (orange)
    pub const WARNING: egui::Color32 = egui::Color32::from_rgb(255, 150, 0);

    /// Success color (green)
    pub const SUCCESS: egui::Color32 = egui::Color32::from_rgb(40, 180, 40);

    // === Text Colors ===

    /// Primary text color for headings and important labels
    pub const TEXT_PRIMARY: egui::Color32 = egui::Color32::from_rgb(220, 220, 225);

    /// Secondary text color for body text and descriptions
    pub const TEXT_SECONDARY: egui::Color32 = egui::Color32::from_rgb(160, 160, 165);

    /// Tertiary text color for captions and hints
    pub const TEXT_TERTIARY: egui::Color32 = egui::Color32::from_rgb(120, 120, 125);

    /// Border color for UI elements
    pub const BORDER: egui::Color32 = egui::Color32::from_rgb(60, 60, 65);

    // === Popup / Modal tokens ===

    /// Muted blue-gray body/subtitle text inside popups
    pub const TEXT_POPUP_BODY: egui::Color32 = egui::Color32::from_rgb(155, 158, 175);

    /// Dark charcoal fill for secondary/cancel buttons inside popups
    pub const BTN_POPUP_DARK: egui::Color32 = egui::Color32::from_rgb(32, 34, 46);
}
