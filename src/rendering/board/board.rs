//! Board creation and rendering
//!
//! Uses batch spawning pattern from Bevy examples (many_sprites.rs, bevymark.rs)
//! to efficiently create all 64 board squares in a single operation.

use crate::game::systems::input::on_square_click;
use crate::game::view_mode::ViewMode;
use crate::input::pointer::{on_square_hover, on_square_unhover};
use crate::rendering::utils::{Square, SquareMaterials};
use bevy::picking::pointer::PointerInteraction;
use bevy::prelude::*;

#[derive(Resource, Component)]
pub struct Board;

pub fn create_board(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    materials: Res<SquareMaterials>,
    view_mode: Res<ViewMode>,
) {
    use crate::core::{DespawnOnExit, GameState};

    // Extract view mode value to avoid move issues in closures
    let is_templeos = *view_mode == ViewMode::TempleOS;

    // Use Rectangle (2D quad) for TempleOS mode, Cuboid (3D box) for standard mode
    // We use a thin Cuboid instead of Plane3d to ensure reliable raycasting/picking
    let boardmesh = if is_templeos {
        // 2D rectangle quad for true 2D board rendering
        meshes.add(Rectangle::new(1.0, 1.0))
    } else {
        // 3D thin box for standard mode - better for picking
        meshes.add(Cuboid::new(1.0, 0.1, 1.0))
    };

    // Choose materials based on view mode
    let (light_mat, dark_mat) = if is_templeos {
        // TempleOS: grey for light squares, white for dark squares
        (
            materials.grey_color.clone(),
            materials.templeos_white.clone(),
        )
    } else {
        // Standard: white for light squares, black for dark squares
        (materials.black_color.clone(), materials.white_color.clone())
    };

    // Pattern from Bevy stress_tests/many_sprites.rs: Collect all squares into Vec, then batch spawn
    // This reduces stack pressure and is more efficient than 64 individual spawn calls
    //
    // Chess coordinate system:
    // - x = file (0-7, a-h), maps to world X
    // - y = rank (0-7, 1-8), maps to world Z
    let squares: Vec<_> = (0..8)
        .flat_map(|rank| {
            // Clone materials and mesh for each row to share across inner closure
            let light_material = light_mat.clone();
            let dark_material = dark_mat.clone();
            let mesh = boardmesh.clone();

            (0..8).map(move |file| {
                // Square uses chess coordinates: x=file, y=rank
                let square = Square::new(file, rank);

                // Use Square::is_white() method for proper checkerboard pattern
                let material = if square.is_white() {
                    light_material.clone()
                } else {
                    dark_material.clone()
                };

                // Generate square name in chess notation (e.g., "Square a1", "Square h8")
                let file_char = (b'a' + file) as char;
                let rank_num = rank + 1;
                let square_name = format!("Square {}{}", file_char, rank_num);

                // World position: X = file, Z = rank
                let world_pos = Vec3::new(file as f32, 0., rank as f32);

                // Bundle all components for this square
                // DespawnOnExit automatically despawns all 64 board squares when exiting Multiplayer
                // For TempleOS mode with Rectangle (2D quad), rotate to lie flat in XZ plane
                let transform = if is_templeos {
                    // Rectangle lies in XY plane by default, rotate -90° around X to lie in XZ plane
                    Transform::from_translation(world_pos)
                        .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2))
                } else {
                    // Cuboid already properly oriented, no rotation needed
                    Transform::from_translation(world_pos)
                };

                (
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material),
                    transform,
                    PointerInteraction::default(),
                    bevy::picking::Pickable::default(), // Required for picking
                    square,
                    Board,
                    Name::new(square_name),
                    DespawnOnExit(GameState::InGame),
                )
            })
        })
        .collect();

    // Spawn all 64 squares in a single batch operation
    // Then attach observers to each (click, hover, unhover)
    for square_bundle in squares {
        commands
            .spawn(square_bundle)
            .observe(on_square_click)
            .observe(on_square_hover)
            .observe(on_square_unhover);
    }
}

pub struct BoardPlugin;

impl Plugin for BoardPlugin {
    fn build(&self, app: &mut App) {
        use super::coordinates::create_coordinate_labels;
        use super::templeos_ui::create_templeos_quote_ui;
        use crate::core::GameState;
        use crate::rendering::setup_templeos_camera;
        use crate::rendering::update_last_move_highlight_system;
        use crate::rendering::update_move_hints_system;
        app.add_systems(
            OnEnter(GameState::InGame),
            (
                create_board,
                create_coordinate_labels,
                setup_templeos_camera,
                create_templeos_quote_ui,
            ),
        )
        .add_systems(
            Update,
            (
                update_move_hints_system.run_if(in_state(GameState::InGame)),
                update_last_move_highlight_system.run_if(in_state(GameState::InGame)),
                crate::rendering::templeos_camera_movement_system
                    .run_if(in_state(GameState::InGame)),
                crate::game::systems::debug_transform::debug_log_transforms
                    .run_if(in_state(GameState::InGame)),
            ),
        );
        // Debug markers removed - they were showing colored spheres on the board corners
        // app.add_systems(
        //     OnEnter(GameState::InGame),
        //     crate::game::systems::debug_visuals::spawn_debug_markers,
        // );
    }
}
