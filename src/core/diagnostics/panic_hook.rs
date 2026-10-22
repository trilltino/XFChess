use std::fs::OpenOptions;
use std::io::Write;
use std::panic;
use std::sync::{Mutex, Once, OnceLock};

use bevy::prelude::*;

use crate::core::{GameState, MenuState};

/// Latest state machine values, readable from the panic hook (which has no World access).
static PANIC_STATE_TRACKER: OnceLock<Mutex<PanicStateInfo>> = OnceLock::new();

#[derive(Debug, Clone, Default)]
struct PanicStateInfo {
    game_state: Option<GameState>,
    menu_state: Option<MenuState>,
}

/// Installs the panic hook that writes `crash_<ts>.log` with the last known states.
/// Safe to call from both `main` (so startup panics are caught) and `CorePlugin`.
pub fn setup_panic_hook() {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(install_panic_hook);
}

fn install_panic_hook() {
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

        let backtrace = std::backtrace::Backtrace::force_capture();
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
            OS: {} {}\n\
            Version: {}\n\
            \n\
            Backtrace:\n\
            {}\n\
            ============================================\n",
            timestamp,
            panic_msg,
            location,
            game_state_str,
            menu_state_str,
            std::env::consts::OS,
            std::env::consts::ARCH,
            env!("CARGO_PKG_VERSION"),
            backtrace_str
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
            eprintln!(
                "[PANIC] XFChess has encountered an error. Crash log: {}",
                log_file.display()
            );
        }
    }));
}

pub fn update_panic_state_tracker(
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
