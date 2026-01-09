use bevy::picking::mesh_picking::MeshPickingPlugin;
use bevy::prelude::*;
use bevy_egui::{EguiGlobalSettings, EguiPlugin};
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

// Core infrastructure
use xfchess::core::{CorePlugin, GameState};

// Game plugins
use xfchess::game::GamePlugin;
// Note: PointerEventsPlugin is added by GamePlugin, not here
use xfchess::rendering::{BoardPlugin, BoardUtils, DynamicLightingPlugin, PiecePlugin};

// State plugins
use xfchess::states::{
    GameOverPlugin, MainMenuPlugin, PausePlugin, PieceViewerPlugin, SettingsPlugin,
};

// Shared systems and resources
use xfchess::audio::apply_master_volume_system;
use xfchess::game::systems::{
    check_and_play_templeos_sound, initialize_engine_from_ecs, initialize_game_sounds,
    initialize_players, play_templeos_sound, reset_game_camera, reset_game_resources,
    setup_game_camera, setup_game_scene, setup_global_scene, spawn_camera_position_ui,
    update_camera_position_ui,
};
use xfchess::persistent_camera::setup_persistent_egui_camera;
use xfchess::rendering::graphics_quality::{
    apply_graphics_quality_camera_system, apply_graphics_quality_lights_system,
    update_graphics_quality_camera_system,
};
use xfchess::ui::fps::fps_ui; // FPS counter for web parity
use xfchess::PersistentEguiCamera;

// Import the hideLoadingScreen function from JavaScript
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = window)]
    fn hideLoadingScreen();
}

/// Log a message to the browser console with a prefix
fn console_log(message: &str) {
    web_sys::console::log_1(&format!("[XFCHESS-WASM] {}", message).into());
}

/// System to hide loading screen when assets are loaded
fn hide_loading_screen_when_ready(
    game_assets: Res<xfchess::assets::GameAssets>,
    mut has_hidden: Local<bool>,
) {
    if game_assets.loaded && !*has_hidden {
        console_log("Assets loaded, hiding loading screen");
        hideLoadingScreen();
        *has_hidden = true;
    }
}

/// Initialize the Bevy chess game for WebAssembly
///
/// This function sets up the full game with all plugins to match the desktop experience.
/// Enhanced with comprehensive debugging for web deployment.
#[wasm_bindgen]
pub fn init_bevy(canvas: HtmlCanvasElement) -> Result<(), JsValue> {
    // Set up panic hook for better error messages in browser console
    console_error_panic_hook::set_once();
    console_log("Panic hook installed");

    // Get canvas dimensions
    let width = canvas.width();
    let height = canvas.height();

    console_log(&format!(
        "Initializing Bevy app with canvas {}x{}",
        width, height
    ));

    // Set the canvas ID - Bevy will automatically find and use it
    canvas.set_id("bevy");
    console_log("Canvas ID set to 'bevy'");

    // Spawn the Bevy app in a separate task
    wasm_bindgen_futures::spawn_local(async move {
        console_log("Starting async Bevy app initialization...");

        let mut app = App::new();

        // ========================================
        // 1. Core Bevy Plugins (WASM specific config)
        // ========================================
        console_log("Step 1: Adding DefaultPlugins...");
        app.add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        canvas: Some("#bevy".into()),
                        fit_canvas_to_parent: true,
                        prevent_default_event_handling: true,
                        ..default()
                    }),
                    ..default()
                })
                .set(AssetPlugin {
                    file_path: "assets".to_string(),
                    meta_check: bevy::asset::AssetMetaCheck::Never,
                    ..default()
                })
                .set(bevy::log::LogPlugin {
                    level: bevy::log::Level::DEBUG,
                    filter: "wgpu=warn,bevy_ecs=info,bevy_render=warn,naga=error".to_string(),
                    ..default()
                }),
        );
        console_log("DefaultPlugins added successfully");

        // ========================================
        // 2. Core Infrastructure
        // ========================================
        console_log("Step 2: Adding CorePlugin and Resources...");
        app.add_plugins(CorePlugin);
        app.init_resource::<PersistentEguiCamera>();
        app.init_resource::<xfchess::assets::LoadingProgress>();
        app.init_resource::<xfchess::assets::GameAssets>();
        console_log("CorePlugin added");

        // ========================================
        // 3. UI Framework & Debugging
        // ========================================
        console_log("Step 3: Adding EguiPlugin and Inspector...");
        app.add_plugins(EguiPlugin::default());

        // Disable auto-creation of primary context (we'll create our own)
        app.add_systems(
            PreStartup,
            |mut egui_settings: ResMut<EguiGlobalSettings>| {
                egui_settings.auto_create_primary_context = false;
            },
        );

        // Add World Inspector for debugging (requested feature)
        app.add_plugins(WorldInspectorPlugin::new());
        console_log("EguiPlugin and Inspector added");

        // ========================================
        // 4. Input Handling
        // ========================================
        console_log("Step 4: Adding input plugins...");
        app.add_plugins(MeshPickingPlugin);
        // Note: PointerEventsPlugin is added by GamePlugin, so we don't add it here
        console_log("Input plugins added");

        // ========================================
        // 5. Rendering Plugins
        // ========================================
        console_log("Step 5: Adding rendering plugins...");
        app.add_plugins(PiecePlugin);
        app.add_plugins(BoardPlugin);
        app.add_plugins(BoardUtils);
        app.add_plugins(DynamicLightingPlugin);
        console_log("Rendering plugins added");

        // ========================================
        // 6. State Plugins
        // ========================================
        console_log("Step 6: Adding state plugins...");
        app.add_plugins(MainMenuPlugin);
        app.add_plugins(SettingsPlugin);
        app.add_plugins(PausePlugin);
        app.add_plugins(GameOverPlugin);
        app.add_plugins(PieceViewerPlugin);
        console_log("State plugins added");

        // ========================================
        // 7. Game Logic
        // ========================================
        console_log("Step 7: Adding GamePlugin...");
        app.add_plugins(GamePlugin);
        console_log("GamePlugin added");

        // ========================================
        // 8. Global Scene Setup
        // ========================================
        console_log("Step 8: Setting up global scene...");
        app.add_systems(PreStartup, setup_persistent_egui_camera);
        app.add_systems(Startup, setup_global_scene);
        console_log("Global scene setup scheduled");

        // ========================================
        // 9. Game State Systems (1:1 with Desktop)
        // ========================================
        console_log("Step 9: Configuring game state systems...");
        app.add_systems(
            OnEnter(GameState::InGame),
            (
                reset_game_resources
                    .before(xfchess::rendering::pieces::create_pieces)
                    .before(xfchess::rendering::board::create_board),
                initialize_players.after(reset_game_resources),
                initialize_game_sounds.after(reset_game_resources),
                play_templeos_sound.after(initialize_game_sounds),
                initialize_engine_from_ecs
                    .after(xfchess::rendering::pieces::create_pieces)
                    .after(xfchess::rendering::board::create_board),
                setup_game_scene,
                setup_game_camera,
                spawn_camera_position_ui,
            )
                .chain(),
        );
        app.add_systems(OnExit(GameState::InGame), reset_game_camera);

        // Updates
        app.add_systems(
            Update,
            (
                hide_loading_screen_when_ready, // Hide loading overlay when assets ready
                handle_pause_input.run_if(in_state(GameState::InGame)),
                check_and_play_templeos_sound.run_if(in_state(GameState::InGame)),
                update_camera_position_ui.run_if(in_state(GameState::InGame)),
                fps_ui.run_if(in_state(GameState::InGame)), // FPS counter (1:1 with desktop)
                // Settings updates
                apply_graphics_quality_camera_system,
                update_graphics_quality_camera_system,
                apply_graphics_quality_lights_system,
                apply_master_volume_system,
            ),
        );

        console_log("Game state systems configured");

        // ========================================
        // 10. Start the App
        // ========================================
        console_log("All plugins added. Starting Bevy app...");
        app.run();
    });

    console_log("Async task spawned, returning from init_bevy");
    Ok(())
}

fn handle_pause_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        console_log("Pausing game");
        next_state.set(GameState::Paused);
    }
}
