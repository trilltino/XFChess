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
    pub const BG_OVERLAY: egui::Color32 = egui::Color32::from_black_alpha(220);

    // === Accent Colors ===

    /// Primary accent (gold - for important buttons and highlights)
    #[allow(dead_code)] // Reserved for future UI styling
    pub const ACCENT_GOLD: egui::Color32 = egui::Color32::from_rgb(218, 165, 32);

    /// Secondary accent (bronze - for secondary actions)
    #[allow(dead_code)] // Reserved for future UI styling
    pub const ACCENT_BRONZE: egui::Color32 = egui::Color32::from_rgb(205, 127, 50);

    /// Success color (green)
    #[allow(dead_code)] // Reserved for future UI styling
    pub const SUCCESS: egui::Color32 = egui::Color32::from_rgb(40, 180, 40);

    /// Warning color (orange)
    #[allow(dead_code)] // Reserved for future UI styling
    pub const WARNING: egui::Color32 = egui::Color32::from_rgb(255, 150, 0);

    /// Error/danger color (red)
    pub const DANGER: egui::Color32 = egui::Color32::from_rgb(220, 50, 50);

    /// Info color (blue)
    #[allow(dead_code)] // Reserved for future UI styling
    pub const INFO: egui::Color32 = egui::Color32::from_rgb(70, 130, 220);

    // === Text Colors ===

    /// Primary text (headings, important text)
    pub const TEXT_PRIMARY: egui::Color32 = egui::Color32::from_rgb(240, 240, 245);

    /// Secondary text (body text)
    pub const TEXT_SECONDARY: egui::Color32 = egui::Color32::from_rgb(200, 200, 205);

    /// Tertiary text (less important, hints)
    pub const TEXT_TERTIARY: egui::Color32 = egui::Color32::from_rgb(150, 150, 155);

    /// Disabled text
    #[allow(dead_code)] // Reserved for future UI styling
    pub const TEXT_DISABLED: egui::Color32 = egui::Color32::from_rgb(100, 100, 105);

    // === Chess-specific Colors ===

    /// White piece color (for UI representation)
    #[allow(dead_code)] // Reserved for future UI styling
    pub const PIECE_WHITE: egui::Color32 = egui::Color32::from_rgb(235, 235, 230);

    /// Black piece color (for UI representation)
    #[allow(dead_code)] // Reserved for future UI styling
    pub const PIECE_BLACK: egui::Color32 = egui::Color32::from_rgb(45, 45, 50);

    /// Light square color (classic chess board)
    #[allow(dead_code)] // Reserved for future UI styling
    pub const SQUARE_LIGHT: egui::Color32 = egui::Color32::from_rgb(238, 238, 210);

    /// Dark square color (classic chess board)
    #[allow(dead_code)] // Reserved for future UI styling
    pub const SQUARE_DARK: egui::Color32 = egui::Color32::from_rgb(118, 150, 86);

    // === Interactive States ===

    /// Hover state color (button hover, etc.)
    #[allow(dead_code)] // Reserved for future UI styling
    pub const HOVER: egui::Color32 = egui::Color32::from_rgb(60, 60, 65);

    /// Active/pressed state color
    #[allow(dead_code)] // Reserved for future UI styling
    pub const ACTIVE: egui::Color32 = egui::Color32::from_rgb(80, 80, 85);

    /// Selected state color
    #[allow(dead_code)] // Reserved for future UI styling
    pub const SELECTED: egui::Color32 = egui::Color32::from_rgb(218, 165, 32); // Gold

    /// Border color
    pub const BORDER: egui::Color32 = egui::Color32::from_rgb(60, 60, 65);
}

/// Helper functions for creating gradients and color variations
#[allow(dead_code)] // Reserved for future UI styling utilities
pub struct ColorUtils;

impl ColorUtils {
    /// Create a semi-transparent version of a color
    #[allow(dead_code)] // Reserved for future UI styling utilities
    pub fn with_alpha(color: egui::Color32, alpha: u8) -> egui::Color32 {
        let [r, g, b, _] = color.to_array();
        egui::Color32::from_rgba_premultiplied(r, g, b, alpha)
    }

    /// Lighten a color by a percentage (0.0 to 1.0)
    #[allow(dead_code)] // Reserved for future UI styling utilities
    pub fn lighten(color: egui::Color32, amount: f32) -> egui::Color32 {
        let [r, g, b, a] = color.to_array();
        let factor = 1.0 + amount;
        egui::Color32::from_rgba_premultiplied(
            (r as f32 * factor).min(255.0) as u8,
            (g as f32 * factor).min(255.0) as u8,
            (b as f32 * factor).min(255.0) as u8,
            a,
        )
    }

    /// Darken a color by a percentage (0.0 to 1.0)
    #[allow(dead_code)] // Reserved for future UI styling utilities
    pub fn darken(color: egui::Color32, amount: f32) -> egui::Color32 {
        let [r, g, b, a] = color.to_array();
        let factor = 1.0 - amount;
        egui::Color32::from_rgba_premultiplied(
            (r as f32 * factor) as u8,
            (g as f32 * factor) as u8,
            (b as f32 * factor) as u8,
            a,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_with_alpha() {
        let color = UiColors::ACCENT_GOLD;
        let transparent = ColorUtils::with_alpha(color, 128);
        assert_eq!(transparent.a(), 128);
    }

    #[test]
    fn test_lighten() {
        let color = egui::Color32::from_rgb(100, 100, 100);
        let lighter = ColorUtils::lighten(color, 0.5);
        assert!(lighter.r() > 100);
        assert!(lighter.g() > 100);
        assert!(lighter.b() > 100);
    }

    #[test]
    fn test_darken() {
        let color = egui::Color32::from_rgb(200, 200, 200);
        let darker = ColorUtils::darken(color, 0.5);
        assert!(darker.r() < 200);
        assert!(darker.g() < 200);
        assert!(darker.b() < 200);
    }
}
