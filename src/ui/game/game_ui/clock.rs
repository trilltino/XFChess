use super::*;

#[derive(Resource, Default)]
pub struct IncrementFlash {
    pub elapsed: f32,
    pub active: bool,
    pub color: bool, // true=white gained, false=black gained
}

impl IncrementFlash {
    pub fn trigger(&mut self, white_gained: bool) {
        self.active = true;
        self.elapsed = 0.0;
        self.color = white_gained;
    }

    pub fn tick(&mut self, dt: f32) {
        if self.active {
            self.elapsed += dt;
            if self.elapsed > 0.8 {
                self.active = false;
            }
        }
    }

    pub fn alpha(&self) -> u8 {
        if self.active {
            let t = (self.elapsed / 0.8).min(1.0);
            ((1.0 - t) * 220.0) as u8
        } else {
            0
        }
    }
}

#[derive(Resource, Default)]
pub struct TimeoutHourglassState {
    pub flagged_player: Option<String>,
    pub elapsed: f32,
}

pub fn timeout_hourglass_system(
    mut hourglass: ResMut<TimeoutHourglassState>,
    mut flag_reader: bevy::prelude::MessageReader<crate::game::events::FlagTimeoutEvent>,
    time: Res<Time>,
) {
    for ev in flag_reader.read() {
        if hourglass.flagged_player.is_none() {
            hourglass.flagged_player = Some(ev.flagged_player.clone());
            hourglass.elapsed = 0.0;
        }
    }
    if hourglass.flagged_player.is_some() {
        hourglass.elapsed += time.delta_secs();
    }
}

pub(crate) fn render_clock_bar(
    ui: &mut egui::Ui,
    time_secs: f32,
    is_active: bool,
    flagged: bool,
    pulse_alpha: u8,
) {
    let low_time = time_secs < 10.0;
    let bg_fill = if is_active && low_time {
        egui::Color32::from_rgba_unmultiplied(130, 18, 18, 230)
    } else {
        egui::Color32::from_rgba_unmultiplied(22, 22, 28, 200)
    };
    let (font_size, color) = if is_active && low_time {
        (24.0_f32, egui::Color32::from_rgb(255, 90, 90))
    } else {
        (
            24.0_f32,
            egui::Color32::from_gray(is_active as u8 * 140 + 100),
        )
    };

    egui::Frame::default()
        .fill(bg_fill)
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.vertical_centered(|ui| {
                let time_str = if flagged {
                    format!("⧖ {}", format_time(time_secs))
                } else {
                    format_time(time_secs)
                };
                let text_color = if flagged {
                    egui::Color32::from_rgba_unmultiplied(255, 130, 0, pulse_alpha)
                } else {
                    color
                };
                ui.label(
                    egui::RichText::new(time_str)
                        .size(font_size)
                        .strong()
                        .color(text_color),
                );
            });
        });
}

pub(super) fn format_time(seconds: f32) -> String {
    let total_seconds = seconds.max(0.0) as u32;
    let minutes = total_seconds / 60;
    let secs = total_seconds % 60;
    format!("{:02}:{:02}", minutes, secs)
}

pub fn increment_flash_system(
    mut flash: ResMut<IncrementFlash>,
    mut move_events: bevy::prelude::MessageReader<crate::game::events::MoveMadeEvent>,
    active_tc: Res<crate::game::resources::active_time_control::ActiveTimeControl>,
    time: Res<Time>,
) {
    flash.tick(time.delta_secs());

    if active_tc.control.increment_seconds() == 0 {
        return;
    }

    for ev in move_events.read() {
        let white_gained = ev.player == "white";
        flash.trigger(white_gained);
    }
}
