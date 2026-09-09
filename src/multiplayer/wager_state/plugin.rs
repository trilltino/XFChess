use crate::GameConfig;
use bevy::prelude::*;

use super::state::WagerState;
use super::ui::wager_ui_system;

pub struct WagerPlugin;

impl Plugin for WagerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WagerState>()
            .add_systems(Startup, initialize_wager_state)
            .add_systems(Update, wager_ui_system);
    }
}

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
