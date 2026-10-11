//! The main menu: the 3D background scene and egui menu ([`menu`]), lobby and
//! connection screens ([`screens`]), modals, wallet-bridge polling and the player's
//! identity.

use crate::assets::{
    check_asset_loading, handle_asset_loading_errors, handle_untyped_asset_loading_errors,
    start_asset_loading,
};
use crate::core::GameState;
use crate::ui::system_params::MainMenuUIContext;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use std::sync::Arc;

mod identity;
mod resources;
mod ui_root;
mod wallet_bridge;
pub use identity::*;
pub use resources::*;
pub(super) use ui_root::*;
pub use wallet_bridge::*;

mod board_animation;
mod famous_games;
pub mod menu;
mod modals;
mod music;
mod screens;
pub mod tournament_replay;

pub use board_animation::BoardAnimator;
pub use menu::MenuPanel;
use menu::{
    menu_escape_system, orbit_camera_system, purge_stale_lights, render_menu, render_wallet_hud,
    setup_menu_fog, spawn_menu_bg_board, spawn_menu_bg_lights, spawn_menu_bg_pieces,
};
use modals::{render_ai_setup_modal, render_controls_popup, render_pgn_input_modal};
use screens::*;

pub struct MainMenuPlugin;

impl Plugin for MainMenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<menu::MenuCameraOrbit>()
            .init_resource::<menu::MenuBgPiecesSpawned>()
            .init_resource::<menu::MenuPanel>()
            .init_resource::<menu::MenuExitConfirm>()
            .init_resource::<menu::MenuFocusMode>()
            .init_resource::<board_animation::BoardAnimator>()
            .init_resource::<music::MenuMusic>()
            .init_resource::<tournament_replay::PgnReplayFetch>()
            .init_resource::<WalletBridgePoller>()
            .init_resource::<FontsLoaded>()
            .add_systems(
                OnEnter(GameState::MainMenu),
                (
                    // Reset panel to Main every time we enter the menu (e.g. returning from a game)
                    |mut panel: ResMut<menu::MenuPanel>,
                     mut exit_confirm: ResMut<menu::MenuExitConfirm>,
                     mut focus_mode: ResMut<menu::MenuFocusMode>| {
                        *panel = menu::MenuPanel::default();
                        exit_confirm.visible = false;
                        focus_mode.active = false;
                    },
                    purge_stale_lights,
                    setup_menu_camera,
                    start_asset_loading,
                    spawn_menu_bg_board,
                    spawn_menu_bg_lights,
                    setup_menu_fog,
                )
                    .chain(),
            )
            .add_systems(
                OnExit(GameState::MainMenu),
                |mut spawned: ResMut<menu::MenuBgPiecesSpawned>,
                 mut fl: ResMut<FontsLoaded>,
                 mut anim: ResMut<board_animation::BoardAnimator>,
                 mut panel: ResMut<menu::MenuPanel>,
                 mut focus_mode: ResMut<menu::MenuFocusMode>,
                 mut global_ambient: ResMut<bevy::light::GlobalAmbientLight>,
                 mut orbit: ResMut<menu::MenuCameraOrbit>,
                 cam: Res<crate::PersistentEguiCamera>,
                 mut projections: Query<&mut Projection, With<Camera3d>>| {
                    spawned.0 = false;
                    fl.0 = false;
                    *anim = board_animation::BoardAnimator::default();
                    *panel = menu::MenuPanel::default();
                    focus_mode.active = false;
                    *global_ambient = bevy::light::GlobalAmbientLight::default();
                    // Restore perspective so in-game cameras don't inherit the
                    // menu's V-toggled orthographic projection.
                    orbit.ortho = false;
                    if let Some(entity) = cam.entity {
                        if let Ok(mut proj) = projections.get_mut(entity) {
                            *proj = Projection::default();
                        }
                    }
                },
            )
            .add_systems(OnEnter(GameState::MainMenu), music::start_menu_music)
            .init_resource::<BrandLogoState>()
            .init_resource::<PlayerColorChoice>()
            .init_resource::<NewsBannerState>()
            .init_resource::<PlayerIdentity>()
            // Leave username unset at startup so the profile picker runs on each launch.
            .init_resource::<crate::assets::GameAssets>()
            .init_resource::<crate::assets::LoadingProgress>()
            .init_resource::<crate::assets::AssetLoadingTimer>()
            .init_resource::<CompetitiveMenuState>()
            .init_resource::<P2PHostState>()
            .init_resource::<crate::states::tournament_menu::TournamentLobbyState>()
            .add_systems(
                EguiPrimaryContextPass,
                (
                    main_menu_ui_wrapper.run_if(in_state(GameState::MainMenu)),
                    render_lobby_selection_popup
                        .run_if(in_state(crate::core::MenuState::LobbySelection))
                        .run_if(in_state(GameState::MainMenu)),
                    // Wallet overlay runs in both menu and in-game so users can connect at any time
                    wallet_connect_overlay_system
                        .run_if(in_state(GameState::MainMenu).or_else(in_state(GameState::InGame))),
                    // Plays a click sound on any egui UI/popup click while in the menu.
                    menu::menu_click_sound
                        .after(main_menu_ui_wrapper)
                        .run_if(in_state(GameState::MainMenu)),
                    // Discrete now-playing widget (track title / skip / mute).
                    music::menu_music_widget.run_if(in_state(GameState::MainMenu)),
                ),
            )
            .add_systems(
                Update,
                (
                    tournament_replay::poll_pgn_replay_fetch,
                    check_asset_loading,
                    handle_asset_loading_errors,
                    handle_untyped_asset_loading_errors,
                    ensure_menu_camera_setup,
                    sync_player_identity_from_wallet,
                    orbit_camera_system,
                    spawn_menu_bg_pieces,
                    try_setup_fonts,
                    // Slide tween for the ambient board pieces.
                    board_animation::animate_menu_pieces,
                    // Finish piece fades before the carousel phase installs a new fade; both
                    // may expire on the same frame.
                    (
                        board_animation::animate_menu_piece_fades,
                        board_animation::animate_ambient_board,
                    )
                        .chain(),
                    menu_escape_system,
                    // Menu playlist: fades between tracks, advances on end / skip.
                    music::drive_menu_music,
                )
                    .chain()
                    .in_set(crate::core::schedule::MenuSystems::Main),
            )
            .add_systems(
                Update,
                // Start Stockfish while the player sets up a game against it; it
                // takes seconds to load.
                crate::game::ai::systems::warm_up_stockfish
                    .run_if(|menu: Res<CompetitiveMenuState>| {
                        menu.show_ai_setup
                            && menu.ai_engine == crate::game::ai::resource::AIEngine::Stockfish
                    })
                    .after(menu_escape_system)
                    .in_set(crate::core::schedule::MenuSystems::Main),
            )
            .add_systems(
                Update,
                // Wallet bridge polling needs to run in both states so the overlay
                // dismisses correctly when the wallet connects during gameplay.
                poll_wallet_bridge
                    .in_set(crate::core::schedule::SessionSet::Wallet)
                    .run_if(in_state(GameState::MainMenu).or_else(in_state(GameState::InGame))),
            );

        // Sync Tauri-bridge pubkey into SolanaIntegrationState so the Solana
        // Wager Lobby sees the wallet as connected immediately after bridge connect.
        #[cfg(feature = "solana")]
        app.add_systems(
            Update,
            sync_bridge_pubkey_to_solana
                .after(poll_wallet_bridge)
                .before(crate::multiplayer::solana::integration::systems::initialize_solana_integration)
                .in_set(crate::core::schedule::SessionSet::Wallet),
        );
    }
}
