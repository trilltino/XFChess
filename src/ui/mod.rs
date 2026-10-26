//! Egui widgets and panels composed by the screens in [`crate::states`] and the
//! in-game view: account, HUD, menus, popups and the shared style system.

pub mod account;
pub mod egui_ext;
pub mod game;
pub mod menus;
pub mod performance;
pub mod responsive;
pub mod spectator_mode;
pub mod styles;
pub mod system_params;

#[cfg(feature = "solana")]
pub use account::solana_panel;

pub use game::game_2d;
pub use game::game_ui;
pub use game::promotion_ui;

pub use menus::compliance_modal;
pub use menus::popup;
pub use menus::profile_onboarding;
pub use menus::stats;
// pub use menus::inspector;

use bevy::prelude::*;
use spectator_mode::SpectatorModePlugin;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(performance::PerformanceMonitorPlugin);
        app.add_plugins(responsive::ResponsiveUiPlugin);
        app.add_plugins(compliance_modal::CompliancePlugin);
        app.add_plugins(profile_onboarding::ProfileOnboardingPlugin);
        app.add_plugins(popup::PopupPlugin);
        app.add_plugins(stats::StatsPlugin);
        app.add_plugins(SpectatorModePlugin);
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::schedule::ScheduleLabel;
    use bevy::prelude::*;
    use bevy_egui::EguiPrimaryContextPass;

    fn systems_in(app: &App, label: impl ScheduleLabel) -> Vec<String> {
        app.get_schedule(label)
            .map(|schedule| {
                schedule
                    .graph()
                    .systems
                    .iter()
                    .map(|(_, system, _)| system.name().to_string())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Windows drawn from `Update` render outside egui's pass in multipass mode;
    /// these four used to be.
    #[test]
    fn overlay_windows_draw_in_the_egui_pass() {
        let mut app = App::new();
        app.add_plugins((
            crate::ui::menus::compliance_modal::CompliancePlugin,
            crate::multiplayer::ui::spectator_overlay::SpectatorOverlayPlugin,
        ));
        #[cfg(feature = "solana")]
        app.add_plugins((
            crate::multiplayer::wager_state::plugin::WagerPlugin,
            crate::multiplayer::solana::integration::plugin::SolanaIntegrationPlugin,
        ));

        let pass = systems_in(&app, EguiPrimaryContextPass);
        let update = systems_in(&app, Update);
        let mut windows = vec!["draw_compliance_modal", "spectator_hud_system"];
        if cfg!(feature = "solana") {
            windows.extend(["wager_ui_system", "profile_view_ui"]);
        }
        for window in windows {
            let is = |name: &String| name.ends_with(&format!("::{window}"));
            assert!(pass.iter().any(is), "{window} should draw in the egui pass");
            assert!(!update.iter().any(is), "{window} must not be in Update");
        }
    }
}
