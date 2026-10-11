use super::*;

#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct Board2DTheme {
    pub dark_sq: egui::Color32,
    pub light_sq: egui::Color32,
}

impl Default for Board2DTheme {
    fn default() -> Self {
        Self::classic()
    }
}

impl Board2DTheme {
    pub fn classic() -> Self {
        Self {
            dark_sq: egui::Color32::from_rgb(181, 136, 99),
            light_sq: egui::Color32::from_rgb(240, 217, 181),
        }
    }
    pub fn green() -> Self {
        Self {
            dark_sq: egui::Color32::from_rgb(118, 150, 86),
            light_sq: egui::Color32::from_rgb(238, 238, 210),
        }
    }
    pub fn blue() -> Self {
        Self {
            dark_sq: egui::Color32::from_rgb(70, 130, 180),
            light_sq: egui::Color32::from_rgb(200, 220, 240),
        }
    }
    pub fn purple() -> Self {
        Self {
            dark_sq: egui::Color32::from_rgb(100, 80, 150),
            light_sq: egui::Color32::from_rgb(220, 210, 240),
        }
    }
    pub fn dark() -> Self {
        Self {
            dark_sq: egui::Color32::from_rgb(60, 60, 70),
            light_sq: egui::Color32::from_rgb(115, 115, 125),
        }
    }

    pub fn sq_color(&self, file: u8, rank: u8) -> egui::Color32 {
        if (file + rank).is_multiple_of(2) {
            self.dark_sq
        } else {
            self.light_sq
        }
    }

    pub fn label_color(&self, file: u8, rank: u8) -> egui::Color32 {
        if (file + rank).is_multiple_of(2) {
            self.light_sq
        } else {
            self.dark_sq
        }
    }
}

pub(super) fn highlight_color(highlight_type: HighlightType) -> egui::Color32 {
    match highlight_type {
        HighlightType::Selected => egui::Color32::from_rgba_unmultiplied(255, 255, 0, 100),
        HighlightType::LegalMove => egui::Color32::from_rgba_unmultiplied(0, 200, 0, 90),
        HighlightType::Capture => egui::Color32::from_rgba_unmultiplied(255, 80, 0, 110),
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) enum HighlightType {
    Selected,
    LegalMove,
    Capture,
}

pub fn sync_board_theme_from_settings(
    settings: Res<crate::core::GameSettings>,
    mut theme: ResMut<Board2DTheme>,
) {
    *theme = theme_from_index(settings.board_theme);
}

pub fn theme_from_index(idx: u8) -> Board2DTheme {
    match idx {
        1 => Board2DTheme::green(),
        2 => Board2DTheme::blue(),
        3 => Board2DTheme::purple(),
        4 => Board2DTheme::dark(),
        _ => Board2DTheme::classic(),
    }
}
