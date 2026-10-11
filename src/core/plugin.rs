use std::fs::OpenOptions;
use std::io::Write;
use std::panic;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use bevy::prelude::*;
use bevy::time::common_conditions::on_timer;

use super::{
    state_lifecycle::{audit_cross_state_leaks, periodic_entity_audit},
    states::log_state_transitions,
    support_bundle::SupportBundleUi,
    GameSettings, GameState, GameStatistics, InGameplay, MenuState, WindowConfig,
};

static PANIC_STATE_TRACKER: OnceLock<Mutex<PanicStateInfo>> = OnceLock::new();

#[derive(Debug, Clone, Default)]
struct PanicStateInfo {
    game_state: Option<GameState>,
    menu_state: Option<MenuState>,
}

pub struct CorePlugin;

impl Plugin for CorePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WindowConfig>();
        super::schedule::configure(app);

        app.init_state::<GameState>()
            .add_sub_state::<MenuState>()
            .add_computed_state::<InGameplay>();

        app.init_resource::<GameStatistics>()
            .init_resource::<super::states::GameMode>()
            .init_resource::<SupportBundleUi>()
            .insert_resource(ClearColor(Color::srgb(0.0, 0.0, 0.0)))
            .register_type::<WindowConfig>()
            .register_type::<GameSettings>()
            .register_type::<GameStatistics>();

        super::settings_persistence::register(app);
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
            super::support_bundle::run_requested_export
                .run_if(|ui: Res<SupportBundleUi>| ui.export_requested),
        );

        // Pause and resume Android audio explicitly; WinitSettings handles background rendering.
        #[cfg(target_os = "android")]
        app.add_systems(Update, crate::android::handle_app_lifecycle);

        // A [GAME-START] marker for every game — online, local, or vs AI —
        // so a support bundle can be cut around a single reported game.
        app.add_systems(
            OnEnter(GameState::InGame),
            |game_mode: Res<super::states::GameMode>| {
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

fn setup_panic_hook() {
    PANIC_STATE_TRACKER.get_or_init(|| Mutex::new(PanicStateInfo::default()));

    panic::set_hook(Box::new(|panic_info| {
        let panic_msg = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "<unknown>".to_string()
        };

        let location = if let Some(loc) = panic_info.location() {
            format!("{}:{}:{}", loc.file(), loc.line(), loc.column())
        } else {
            "<unknown>".to_string()
        };

        // Get state information
        let mut game_state_str = "<unknown>".to_string();
        let mut menu_state_str = "<not in menu>".to_string();
        if let Some(tracker) = PANIC_STATE_TRACKER.get() {
            if let Ok(state_info) = tracker.lock() {
                if let Some(game_state) = state_info.game_state {
                    game_state_str = format!("{:?}", game_state);
                }
                if let Some(menu_state) = state_info.menu_state {
                    menu_state_str = format!("{:?}", menu_state);
                }
            }
        }

        let backtrace = std::backtrace::Backtrace::capture();
        let backtrace_str = format!("{}", backtrace);

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let panic_report = format!(
            "PANIC DETECTED [{}]\n\
            ============================================\n\
            Message: {}\n\
            Location: {}\n\
            GameState: {}\n\
            MenuState: {}\n\
            \n\
            Backtrace:\n\
            {}\n\
            ============================================\n",
            timestamp, panic_msg, location, game_state_str, menu_state_str, backtrace_str
        );

        eprintln!("\n{}", panic_report);

        // Write beside game.log so support bundles include this log.
        let logs_dir = crate::multiplayer::network::identity::log_dir();

        let log_file = logs_dir.join(format!("crash_{}.log", timestamp));
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&log_file)
        {
            let _ = writeln!(file, "{}", panic_report);
            eprintln!("[PANIC] Crash log written to: {:?}", log_file);
        }
    }));
}

fn log_runtime_banner() {
    #[cfg(debug_assertions)]
    let build = "debug";
    #[cfg(not(debug_assertions))]
    let build = "release";
    info!(
        "[runtime] XFChess v{} {} {} {} — logs → {}",
        super::updates::CURRENT_VERSION,
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

fn update_panic_state_tracker(
    game_state: Option<Res<State<GameState>>>,
    menu_state: Option<Res<State<MenuState>>>,
) {
    if let Some(tracker) = PANIC_STATE_TRACKER.get() {
        if let Ok(mut state_info) = tracker.lock() {
            if let Some(game_state_res) = game_state {
                state_info.game_state = Some(*game_state_res.get());
            }
            if let Some(menu_state_res) = menu_state {
                state_info.menu_state = Some(*menu_state_res.get());
            }
        }
    }
}
