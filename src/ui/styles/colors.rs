use bevy_egui::egui;

pub struct UiColors;

impl UiColors {
    // === Background Colors ===

    pub const BG_DARK: egui::Color32 = egui::Color32::from_rgb(20, 20, 25);

    pub const BG_MID: egui::Color32 = egui::Color32::from_rgb(30, 30, 35);

    pub const BG_LIGHT: egui::Color32 = egui::Color32::from_rgb(40, 40, 45);

    pub const BG_OVERLAY: egui::Color32 = egui::Color32::from_black_alpha(150);

    // === Accent Colors ===

    pub const ACCENT_GOLD: egui::Color32 = egui::Color32::from_rgb(218, 165, 32);

    pub const ACCENT: egui::Color32 = egui::Color32::from_rgb(173, 92, 47); // #ad5c2f

    pub const DANGER: egui::Color32 = egui::Color32::from_rgb(220, 50, 50);

    pub const WARNING: egui::Color32 = egui::Color32::from_rgb(255, 150, 0);

    pub const SUCCESS: egui::Color32 = egui::Color32::from_rgb(40, 180, 40);

    // === Text Colors ===

    pub const TEXT_PRIMARY: egui::Color32 = egui::Color32::from_rgb(220, 220, 225);

    pub const TEXT_SECONDARY: egui::Color32 = egui::Color32::from_rgb(160, 160, 165);

    pub const TEXT_TERTIARY: egui::Color32 = egui::Color32::from_rgb(120, 120, 125);

    pub const BORDER: egui::Color32 = egui::Color32::from_rgb(60, 60, 65);

    // === Popup / Modal tokens ===

    pub const TEXT_POPUP_BODY: egui::Color32 = egui::Color32::from_rgb(155, 158, 175);

    pub const BTN_POPUP_DARK: egui::Color32 = egui::Color32::from_rgb(32, 34, 46);
}
