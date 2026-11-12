//! Profile- and wallet-keyed social graph (kept by the backend) and presence.
//! All network work runs off the frame thread; views consume cached state.
use bevy::prelude::*;
use std::time::Instant;
mod presence;
mod resources;
mod service;
pub mod ui;
pub use resources::*;
pub use service::{FriendAction, FriendsState};

pub struct SocialPlugin;
impl Plugin for SocialPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FriendsState>()
            .init_resource::<ui::FriendsUiState>()
            .init_resource::<LobbyFilterConfig>()
            .init_resource::<LobbyFetchState>()
            .init_resource::<BackendRegion>()
            .init_resource::<OnlinePlayersState>()
            .init_resource::<OpponentLivenessState>()
            .add_systems(
                Update,
                (
                    service::sync_identity,
                    service::update_friends,
                    presence::tick_presence_sync,
                    presence::fetch_backend_region_once,
                    presence::check_opponent_presence,
                )
                    .chain()
                    .in_set(crate::core::schedule::SessionSet::Social),
            )
            .add_systems(
                bevy_egui::EguiPrimaryContextPass,
                ui::friends_ui
                    .after(crate::states::main_menu::dialogs::MenuUiRoot)
                    .after(crate::ui::game::game_ui::game_status_ui),
            )
            .add_systems(
                OnExit(crate::core::GameState::InGame),
                |mut ui: ResMut<ui::FriendsUiState>| {
                    ui.open = false;
                },
            );
    }
}
