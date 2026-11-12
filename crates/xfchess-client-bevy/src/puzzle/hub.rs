//! The main-menu puzzle panel: pick a kind (Rated, Daily, Lichess, Earn), a theme
//! and a difficulty, then start.

use bevy::prelude::*;
use bevy_egui::egui;

use super::api::{self, Difficulty, PuzzleKind, PuzzleRequest, Theme};
use super::session::{PuzzlePhase, PuzzleSession};
use crate::core::net_task::{Pending, Poll};

/// The hub's choices, kept across menu visits.
#[derive(Resource, Default)]
pub struct PuzzleHub {
    pub kind: PuzzleKind,
    /// Selected theme key; `None` is any theme.
    pub theme: Option<String>,
    pub difficulty: Difficulty,
    themes: Vec<Theme>,
    themes_loading: Option<Pending<Result<Vec<Theme>, String>>>,
    themes_failed: bool,
}

impl PuzzleHub {
    fn load_themes_once(&mut self) {
        if self.themes.is_empty() && self.themes_loading.is_none() && !self.themes_failed {
            self.themes_loading = Some(Pending::spawn("puzzle-themes", api::fetch_themes));
        }
    }
}

pub(super) fn poll_themes(mut hub: ResMut<PuzzleHub>) {
    let Some(pending) = hub.themes_loading.as_ref() else {
        return;
    };
    let result = match pending.poll() {
        Poll::Running => return,
        Poll::Ready(result) => result,
        Poll::Lost => Err("theme request failed".into()),
    };
    hub.themes_loading = None;
    match result {
        // `mix` is the server's "any theme", already covered by `None`.
        Ok(themes) => hub.themes = themes.into_iter().filter(|t| t.key != "mix").collect(),
        Err(e) => {
            warn!("[puzzle] could not load themes: {e}");
            hub.themes_failed = true;
        }
    }
}

/// What the player chose in the hub this frame.
pub enum HubAction {
    /// Start a puzzle. `wallet` is empty for a guest; the caller fills in the
    /// guest's stand-in id (only free modes can be started without a wallet).
    Start(PuzzleRequest),
    LinkLichess(String),
}

/// Draws the hub. `width` is the menu column width.
pub fn show_hub(
    ui: &mut egui::Ui,
    hub: &mut PuzzleHub,
    session: &PuzzleSession,
    wallet: Option<&str>,
    width: f32,
) -> Option<HubAction> {
    hub.load_themes_once();
    let mut action = None;

    ui.horizontal_wrapped(|ui| {
        for kind in PuzzleKind::ALL {
            ui.selectable_value(&mut hub.kind, kind, kind.label());
        }
    });
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(describe(hub.kind))
            .small()
            .color(egui::Color32::from_rgba_unmultiplied(200, 200, 215, 180)),
    );
    ui.add_space(8.0);

    if hub.kind.is_tunable() {
        let theme_label = hub
            .theme
            .as_ref()
            .and_then(|key| hub.themes.iter().find(|t| &t.key == key))
            .map_or("Any theme", |t| t.label.as_str())
            .to_owned();
        egui::ComboBox::from_id_salt("puzzle_theme")
            .width(width)
            .selected_text(theme_label)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut hub.theme, None, "Any theme");
                for theme in &hub.themes {
                    ui.selectable_value(&mut hub.theme, Some(theme.key.clone()), &theme.label);
                }
            });
        egui::ComboBox::from_id_salt("puzzle_difficulty")
            .width(width)
            .selected_text(hub.difficulty.label())
            .show_ui(ui, |ui| {
                for d in Difficulty::ALL {
                    ui.selectable_value(&mut hub.difficulty, d, d.label());
                }
            });
        ui.add_space(8.0);
    }

    let wallet = wallet.filter(|w| !w.trim().is_empty());
    let allowed = wallet.is_some() || !hub.kind.needs_wallet();
    let busy = session.is_busy();
    let start_label = if busy { "Loading…" } else { "Start" };
    let start = egui::Button::new(start_label).min_size(egui::vec2(width, 28.0));
    if ui.add_enabled(allowed && !busy, start).clicked() {
        action = Some(HubAction::Start(PuzzleRequest {
            kind: hub.kind,
            wallet: wallet.unwrap_or_default().to_owned(),
            theme: hub.theme.clone().filter(|_| hub.kind.is_tunable()),
            difficulty: hub.difficulty,
        }));
    }
    if !allowed {
        ui.label(egui::RichText::new("Connect a wallet to use this mode").small());
    }

    if hub.kind == PuzzleKind::Lichess {
        ui.add_space(6.0);
        let link = egui::Button::new("Connect Lichess").min_size(egui::vec2(width, 24.0));
        if let Some(w) = wallet {
            if ui
                .add(link)
                .on_hover_text(
                    "Opens Lichess in your browser; already linked accounts can just start",
                )
                .clicked()
            {
                action = Some(HubAction::LinkLichess(w.to_owned()));
            }
        }
    }

    if session.phase == PuzzlePhase::Idle && !session.status.is_empty() {
        ui.add_space(6.0);
        ui.label(
            egui::RichText::new(&session.status)
                .small()
                .color(egui::Color32::from_rgb(230, 170, 90)),
        );
    }
    action
}

fn describe(kind: PuzzleKind) -> &'static str {
    match kind {
        PuzzleKind::Rated => "Tactics from the Lichess puzzle database, matched to your rating.",
        PuzzleKind::Daily => "One puzzle a day, the same for everyone.",
        PuzzleKind::Lichess => {
            "Puzzles from your linked Lichess account. Results count on Lichess too."
        }
        PuzzleKind::Earn => "Puzzles with a SOL bounty, paid to your wallet when solved.",
    }
}
