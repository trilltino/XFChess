use bevy::{
    asset::AssetMetaCheck,
    audio::{AudioPlugin, Volume},
    log::LogPlugin,
    prelude::*,
};
use bevy_egui::EguiPlugin;

mod assets;
mod core;
mod engine;
mod game;
mod input;
mod multiplayer;
mod persistent_camera;
mod rendering;
mod singleplayer;
#[cfg(feature = "solana")]
mod solana;
mod states;
mod ui;

pub use persistent_camera::PersistentEguiCamera;

#[tokio::main]
async fn main() {
    let handle = tokio::runtime::Handle::current();
    let mut app = App::new();
    app.insert_resource(multiplayer::TokioRuntime(handle))
        .init_resource::<PersistentEguiCamera>()
        .add_systems(PreStartup, persistent_camera::setup_persistent_egui_camera);

    // Add core plugins
    app.add_plugins(
        DefaultPlugins
            .set(AssetPlugin {
                // Wasm builds will check for meta files (that don't exist) if this isn't set
                meta_check: AssetMetaCheck::Never,
                // Use the project root assets folder so the game works regardless of
                // which directory the executable is launched from.
                #[cfg(not(target_arch = "wasm32"))]
                file_path: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("assets")
                    .to_string_lossy()
                    .into_owned(),
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "XFChess".to_string(),
                    fit_canvas_to_parent: true,
                    prevent_default_event_handling: false,
                    ..default()
                }),
                ..default()
            })
            // Using volume from default to reduce popping sounds
            .set(AudioPlugin {
                global_volume: GlobalVolume {
                    volume: Volume::Linear(0.3),
                },
                ..default()
            })
            // Disable console logging in release mode to reduce WASM size
            .set(LogPlugin {
                filter: if cfg!(debug_assertions) {
                    "info,wgpu_core=warn,wgpu_hal=warn,xfchess=debug".to_string()
                } else {
                    "error".to_string()
                },
                ..default()
            }),
    )
    .add_plugins(EguiPlugin::default());

    // Add custom plugins
    app.add_plugins((
        core::CorePlugin,
        game::GamePlugin,
        rendering::RenderingPlugin,
        ui::UiPlugin,
        input::InputPlugin,
        states::main_menu::MainMenuPlugin,
        states::multiplayer_menu::MultiplayerMenuPlugin,
        singleplayer::SingleplayerPlugin,
        #[cfg(feature = "solana")]
        solana::SolanaPlugin,
        multiplayer::MultiplayerPlugin,
        #[cfg(feature = "solana")]
        multiplayer::ephemeral_mvp_plugin::EphemeralMvpPlugin,
    ));

    // Run the app
    app.run();
}
