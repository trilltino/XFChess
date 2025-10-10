use bevy::prelude::*;
use bevy_egui::EguiPlugin;

// Module declarations
mod core;
mod rendering;
mod ui;
mod input;
mod game;

// Imports
use core::{GameState, LaunchMenu};
use rendering::*;
use ui::*;
use input::*;
use game::GamePlugin;

const WINDOW_WIDTH: u32 = 1366;
const WINDOW_HEIGHT: u32 = 768;

fn main() {
    let window = Window {
        resolution: (WINDOW_WIDTH as f32, WINDOW_HEIGHT as f32).into(),
        ..default()
    };
    let primary_window = Some(window);

    App::new()
        // Core plugins
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window,
            ..default()
        }))
        .add_plugins(EguiPlugin { enable_multipass_for_primary_context: false })

        // Game state
        .init_state::<GameState>()
        .add_computed_state::<LaunchMenu>()

        // Game systems
        .add_plugins(UIPlugin)
        .add_plugins(MeshPickingPlugin)
        .add_plugins(PiecePlugin)
        .add_plugins(GamePlugin)
        .add_plugins(BoardPlugin)
        .add_plugins(BoardUtils)
        .add_plugins(PointerEventsPlugin)
        .add_plugins(Launchmenu {
            state: GameState::LaunchMenu,
        })

        // Startup systems
        .add_systems(OnEnter(GameState::LaunchMenu), setup_launch_camera)
        .add_systems(OnEnter(GameState::Multiplayer), setup_game_camera)
        .add_systems(Startup, setup)
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Lighting
    commands.spawn((
        PointLight {
            shadows_enabled: true,
            intensity: 100000.0,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0),
    ));

    // Background
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(2.0, 1.0, 1.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Srgba::hex("000000").unwrap().into(),
            unlit: true,
            cull_mode: None,
            ..default()
        })),
        Transform::from_scale(Vec3::splat(1_000_000.0)),
    ));
}

fn setup_game_camera(mut commands: Commands) {
    // Game camera for chess board view
    commands.spawn((
        Camera3d::default(),
        Transform::from_matrix(Mat4::from_rotation_translation(
            Quat::from_xyzw(-0.3, -0.5, -0.3, 0.5).normalize(),
            Vec3::new(-7.0, 20.0, 4.0),
        )),
    ));
}
