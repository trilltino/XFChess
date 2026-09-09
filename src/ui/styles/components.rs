use super::colors::UiColors;
use bevy_egui::egui;

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

pub struct Layout;

impl Layout {
    pub const SECTION_SPACING: f32 = 30.0;

    pub const ITEM_SPACING: f32 = 15.0;

    pub const SMALL_SPACING: f32 = 8.0;

    pub const SIDE_PANEL_WIDTH: f32 = 260.0;

    pub fn section_space(ui: &mut egui::Ui) {
        ui.add_space(Self::SECTION_SPACING);
    }

    pub fn item_space(ui: &mut egui::Ui) {
        ui.add_space(Self::ITEM_SPACING);
    }

    pub fn small_space(ui: &mut egui::Ui) {
        ui.add_space(Self::SMALL_SPACING);
    }
}

pub struct ModernButton;

impl ModernButton {
    pub fn primary(ui: &mut egui::Ui, text: &str) -> egui::Response {
        let button = egui::Button::new(text)
            .fill(egui::Color32::from_rgba_premultiplied(40, 40, 45, 200))
            .stroke(egui::Stroke::new(
                1.0,
                egui::Color32::from_rgb(218, 165, 32),
            ));
        ui.add(button)
    }

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

pub struct StyledButton;

impl StyledButton {
    pub fn primary(ui: &mut egui::Ui, text: &str) -> egui::Response {
        let button = egui::Button::new(text)
            .fill(UiColors::ACCENT_GOLD)
            .stroke(egui::Stroke::new(1.0, UiColors::BORDER));
        ui.add(button)
    }

    pub fn small(ui: &mut egui::Ui, text: &str) -> egui::Response {
        let button = egui::Button::new(text)
            .fill(UiColors::BG_LIGHT)
            .stroke(egui::Stroke::new(1.0, UiColors::BORDER));
        ui.add(button)
    }

    pub fn danger(ui: &mut egui::Ui, text: &str) -> egui::Response {
        let button = egui::Button::new(text)
            .fill(UiColors::DANGER)
            .stroke(egui::Stroke::new(1.0, UiColors::BORDER));
        ui.add(button)
    }

    pub fn secondary(ui: &mut egui::Ui, text: &str) -> egui::Response {
        let button = egui::Button::new(text)
            .fill(UiColors::BG_LIGHT)
            .stroke(egui::Stroke::new(1.0, UiColors::BORDER));
        ui.add(button)
    }

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

pub struct StyledPanel;

impl StyledPanel {
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

    pub fn popup_alpha(alpha: u8) -> egui::Frame {
        egui::Frame {
            fill: egui::Color32::from_rgba_unmultiplied(8, 10, 18, alpha),
            corner_radius: egui::CornerRadius::same(14),
            stroke: egui::Stroke::NONE,
            inner_margin: egui::Margin::same(24),
            ..egui::Frame::NONE
        }
    }

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
