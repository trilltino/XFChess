use bevy::picking::mesh_picking::MeshPickingPlugin;
use bevy::prelude::*;
use bevy_egui::{
    EguiContext, EguiGlobalSettings, EguiMultipassSchedule, EguiPlugin, EguiPrimaryContextPass,
    PrimaryEguiContext,
};
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

// Core infrastructure
use xfchess::core::{CorePlugin, DespawnOnExit, GameState};

// Game plugins
use xfchess::game::GamePlugin;
use xfchess::input::PointerEventsPlugin;
use xfchess::rendering::{BoardPlugin, BoardUtils, DynamicLightingPlugin, PiecePlugin};

// State plugins
use xfchess::states::{
    GameOverPlugin, MainMenuPlugin, PausePlugin, PieceViewerPlugin, SettingsPlugin,
};

// Persistent camera (re-exported from xfchess lib)
use xfchess::PersistentEguiCamera;

/// Log a message to the browser console with a prefix
fn console_log(message: &str) {
    web_sys::console::log_1(&format!("[XFCHESS-WASM] {}", message).into());
}

/// Log an error to the browser console with a prefix
fn console_error(message: &str) {
    web_sys::console::error_1(&format!("[XFCHESS-WASM ERROR] {}", message).into());
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
                        prevent_default_event_handling: false,
                        ..default()
                    }),
                    ..default()
                })
                .set(AssetPlugin {
                    file_path: "assets".to_string(),
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
        // 2. Core Infrastructure (State Machine, Settings)
        // ========================================
        console_log("Step 2: Adding CorePlugin...");
        app.add_plugins(CorePlugin);
        console_log("CorePlugin added");

        // Initialize PersistentEguiCamera resource (critical for UI)
        console_log("Step 2b: Initializing PersistentEguiCamera resource...");
        app.init_resource::<PersistentEguiCamera>();
        console_log("PersistentEguiCamera resource initialized");

        // ========================================
        // 3. UI Framework
        // ========================================
        console_log("Step 3: Adding EguiPlugin...");
        app.add_plugins(EguiPlugin::default());

        // Disable auto-creation of primary context (we'll create our own)
        app.add_systems(
            PreStartup,
            |mut egui_settings: ResMut<EguiGlobalSettings>| {
                egui_settings.auto_create_primary_context = false;
            },
        );
        console_log("EguiPlugin added");

        // ========================================
        // 4. Input Handling (CRITICAL for piece clicks)
        // ========================================
        console_log("Step 4: Adding input plugins...");
        app.add_plugins(MeshPickingPlugin);
        app.add_plugins(PointerEventsPlugin);
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
        // 6. State Plugins (Menus, Settings, etc.)
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
        // 8. Global Scene Setup (web-specific initialization)
        // ========================================
        console_log("Step 8: Setting up global scene...");

        // Setup persistent egui camera (runs in PreStartup)
        app.add_systems(PreStartup, setup_persistent_egui_camera);

        // Setup global scene elements (ambient light, background)
        app.add_systems(Startup, setup_global_scene_web);

        console_log("Global scene setup scheduled");

        // ========================================
        // 9. Game State Systems
        // ========================================
        console_log("Step 9: Configuring game state systems...");
        app.add_systems(OnEnter(GameState::InGame), setup_game_scene_web);
        console_log("Game state systems configured");

        // ========================================
        // 10. Start the App
        // ========================================
        console_log("All plugins added. Starting Bevy app...");
        console_log("====================================");
        console_log("If you see this, initialization succeeded!");
        console_log("If the game doesn't appear, check for panics above.");
        console_log("====================================");

        // Run the app
        app.run();
    });

    console_log("Async task spawned, returning from init_bevy");
    Ok(())
}

/// Setup persistent EGUI camera for web (matches main.rs setup)
fn setup_persistent_egui_camera(
    mut commands: Commands,
    mut persistent_camera: ResMut<PersistentEguiCamera>,
) {
    console_log("Setting up persistent EGUI camera...");

    let camera_entity = commands
        .spawn((
            Camera3d::default(),
            Transform::from_xyz(0.0, 5.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
            EguiContext::default(),
            PrimaryEguiContext,
            EguiMultipassSchedule::new(EguiPrimaryContextPass),
            Name::new("Persistent Egui Camera"),
        ))
        .id();

    persistent_camera.entity = Some(camera_entity);
    console_log(&format!(
        "Persistent EGUI camera created with entity ID: {:?}",
        camera_entity
    ));
}

/// Setup global scene for web (matches main.rs setup_global_scene)
fn setup_global_scene_web(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    console_log("Setting up global scene (background, ambient light)...");

    // Global background (dark space-like environment)
    let background_color = Color::srgba(0.04, 0.04, 0.08, 1.0); // Very dark blue

    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(2.0, 1.0, 1.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: background_color,
            unlit: true,
            cull_mode: None,
            ..default()
        })),
        Transform::from_scale(Vec3::splat(1_000_000.0)),
        Name::new("Global Background"),
    ));

    // Global ambient light
    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.3, 0.3, 0.35),
        brightness: 300.0,
        affects_lightmapped_meshes: true,
    });

    console_log("Global scene setup complete");
}

/// Setup game scene for web (simplified version of main.rs setup_game_scene)
fn setup_game_scene_web(mut commands: Commands) {
    console_log("Setting up in-game scene...");

    // Set background color
    commands.insert_resource(ClearColor(Color::srgb(0.0, 0.0, 0.0)));

    // Main directional light
    commands.spawn((
        DirectionalLight {
            illuminance: 8000.0,
            shadows_enabled: true,
            color: Color::srgb(1.0, 0.98, 0.95),
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(
            EulerRot::XYZ,
            -std::f32::consts::FRAC_PI_4,
            std::f32::consts::FRAC_PI_4,
            0.0,
        )),
        DespawnOnExit(GameState::InGame),
        Name::new("Main Directional Light"),
    ));

    // Fill light
    commands.spawn((
        PointLight {
            intensity: 500_000.0,
            color: Color::srgb(0.9, 0.9, 1.0),
            shadows_enabled: false,
            range: 30.0,
            ..default()
        },
        Transform::from_xyz(-10.0, 10.0, 10.0),
        DespawnOnExit(GameState::InGame),
        Name::new("Fill Light"),
    ));

    console_log("In-game scene setup complete");
}
