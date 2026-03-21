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
    asset_server: Res<AssetServer>,
    view_mode: Res<ViewMode>,
) {
    use crate::core::{DespawnOnExit, GameState};

    let is_templeos = *view_mode == ViewMode::TempleOS;

    // Use Rectangle (2D quad) for TempleOS mode, Cuboid (3D box) for standard mode
    let boardmesh = if is_templeos {
        meshes.add(Rectangle::new(1.0, 1.0))
    } else {
        meshes.add(Cuboid::new(1.0, 0.1, 1.0))
    };

    // Load materials from wooden_chess_board.glb like the reference
    let mat_light: Handle<StandardMaterial> =
        asset_server.load("models/wooden_chess_board.glb#Material0");
    let mat_dark: Handle<StandardMaterial> =
        asset_server.load("models/wooden_chess_board.glb#Material1");

    let squares: Vec<_> = (0..8)
        .flat_map(|rank| {
            let mesh = boardmesh.clone();
            let mat_light_row = mat_light.clone();
            let mat_dark_row = mat_dark.clone();

            (0..8).map(move |file| {
                let square = Square::new(file, rank);
                let is_white_square = square.is_white();

                // Use GLB materials like reference: Material0 for light, Material1 for dark
                let base_mat = if is_white_square {
                    mat_light_row.clone()
                } else {
                    mat_dark_row.clone()
                };

                let file_char = (b'a' + file) as char;
                let rank_num = rank + 1;
                let square_name = format!("Square {}{}", file_char, rank_num);

                let world_pos = Vec3::new(file as f32, 0., rank as f32);

                let transform = if is_templeos {
                    Transform::from_translation(world_pos)
                        .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2))
                } else {
                    Transform::from_translation(world_pos)
                };

                (
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(base_mat),
                    transform,
                    PointerInteraction::default(),
                    bevy::picking::Pickable::default(),
                    square,
                    Board,
                    Name::new(square_name),
                    DespawnOnExit(GameState::InGame),
                )
            })
        })
        .collect();

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
