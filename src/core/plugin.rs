use std::time::Duration;

use bevy::prelude::*;
use bevy::time::common_conditions::on_timer;

use super::{
    diagnostics::{
        panic_hook::{setup_panic_hook, update_panic_state_tracker},
        support_bundle::SupportBundleUi,
    },
    state::{
        audit::{audit_cross_state_leaks, periodic_entity_audit},
        log_state_transitions,
    },
    GameSettings, GameState, GameStatistics, InGameplay, MenuState, WindowConfig,
};

pub struct CorePlugin;

impl Plugin for CorePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WindowConfig>();
        super::schedule::configure(app);

        app.init_state::<GameState>()
            .add_sub_state::<MenuState>()
            .add_computed_state::<InGameplay>();

        app.init_resource::<GameStatistics>()
            .init_resource::<super::state::GameMode>()
            .init_resource::<SupportBundleUi>()
            .insert_resource(ClearColor(Color::srgb(0.0, 0.0, 0.0)))
            .register_type::<WindowConfig>()
            .register_type::<GameSettings>()
            .register_type::<GameStatistics>();

        super::settings::persistence::register(app);
        app.add_systems(Startup, log_runtime_banner);

        // Diagnostics run only when there is something to report.
        app.add_systems(
            Update,
            (
                log_state_transitions.run_if(on_message::<StateTransitionEvent<GameState>>),
                log_menu_state.run_if(state_changed::<MenuState>),
                update_panic_state_tracker
                    .run_if(state_changed::<GameState>.or_else(state_changed::<MenuState>)),
                audit_cross_state_leaks.run_if(state_changed::<GameState>),
                periodic_entity_audit.run_if(on_timer(Duration::from_secs(10))),
            ),
        );

        #[cfg(not(target_os = "android"))]
        app.add_systems(
            Update,
            super::diagnostics::support_bundle::run_requested_export
                .run_if(|ui: Res<SupportBundleUi>| ui.export_requested),
        );

        // Pause and resume Android audio explicitly; WinitSettings handles background rendering.
        #[cfg(target_os = "android")]
        app.add_systems(Update, crate::android::handle_app_lifecycle);

        // A [GAME-START] marker for every game — online, local, or vs AI —
        // so a support bundle can be cut around a single reported game.
        app.add_systems(
            OnEnter(GameState::InGame),
            |game_mode: Res<super::state::GameMode>| {
                info!("[GAME-START] mode={:?}", *game_mode);
            },
        );
    }

    fn finish(&self, _app: &mut App) {
        // Set up panic hook in finish() to ensure it's configured
        // after all plugins are built but before the app runs
        #[cfg(not(target_arch = "wasm32"))]
        setup_panic_hook();

        // Enable full backtraces for debugging
        #[cfg(not(target_arch = "wasm32"))]
        std::env::set_var("RUST_BACKTRACE", "full");
    }
}

fn log_runtime_banner() {
    #[cfg(debug_assertions)]
    let build = "debug";
    #[cfg(not(debug_assertions))]
    let build = "release";
    info!(
        "[runtime] XFChess v{} {} {} {} — logs → {}",
        super::update_check::CURRENT_VERSION,
        std::env::consts::OS,
        std::env::consts::ARCH,
        build,
        crate::multiplayer::network::identity::log_dir().display()
    );
}

fn log_menu_state(state: Option<Res<State<MenuState>>>) {
    if let Some(state) = state {
        debug!("[STATE] Menu: {:?}", state.get());
    }
}
