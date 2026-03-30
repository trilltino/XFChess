use bevy::prelude::*;
use crate::GameConfig;

use super::state::WagerState;
use super::ui::wager_ui_system;

/// Plugin for wager integration
pub struct WagerPlugin;

impl Plugin for WagerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WagerState>()
            .add_systems(Startup, initialize_wager_state)
            .add_systems(Update, wager_ui_system);
    }
}

/// Initialize wager state from CLI config
fn initialize_wager_state(config: Res<GameConfig>, mut wager_state: ResMut<WagerState>) {
    *wager_state = WagerState::from_config(&config);

    if wager_state.is_loaded {
        info!(
            "[WagerState] Loaded wager: {} | Pot: {}",
            wager_state.wager_display(),
            wager_state.pot_display()
        );
    }
}
