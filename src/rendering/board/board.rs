//! Board creation and rendering
//!
//! Uses batch spawning pattern from Bevy examples (many_sprites.rs, bevymark.rs)
//! to efficiently create all 64 board squares in a single operation.

use crate::game::systems::input::on_square_click;
use crate::game::view_mode::ViewMode;
use crate::input::pointer::{on_square_hover, on_square_unhover};
use crate::rendering::utils::Square;
use bevy::picking::pointer::PointerInteraction;
use bevy::prelude::*;

#[derive(Resource, Component)]
pub struct Board;

/// Component marking a 3D visual element of a board square
#[derive(Component)]
pub struct BoardSquare3DVisual;

/// Component marking a 2D visual element of a board square
#[derive(Component)]
pub struct BoardSquare2DVisual;

pub fn create_board(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    view_mode: Res<ViewMode>,
    square_materials: Res<crate::rendering::utils::SquareMaterials>,
) {
    use crate::core::{DespawnOnExit, GameState};

    let _is_templeos = *view_mode == ViewMode::TempleOS;

    // Use Rectangle (2D quad) for TempleOS mode, Cuboid (3D box) for standard mode
    let boardmesh_3d = meshes.add(Cuboid::new(1.0, 0.1, 1.0));
    let boardmesh_2d = meshes.add(Rectangle::new(1.0, 1.0));
    
    // Lichess-style colors for 2D board
    // Light: #f0d9b5, Dark: #b58863
    let mat_2d_light = materials.add(StandardMaterial {
        base_color: Color::srgb(0.94, 0.85, 0.71),
        unlit: true,
        ..default()
    });
    let mat_2d_dark = materials.add(StandardMaterial {
        base_color: Color::srgb(0.71, 0.53, 0.39),
        unlit: true,
        ..default()
    });

    // Use SquareMaterials resource for consistent 3D coloring
    let mat_light = square_materials.white_color.clone(); // White squares use Green (standard terminology might be flipped)
    let mat_dark = square_materials.black_color.clone();  // Black squares use Cream

    let squares: Vec<_> = (0..8)
        .flat_map(|rank| {
            let mesh_3d = boardmesh_3d.clone();
            let mesh_2d = boardmesh_2d.clone();
            let mat_light_row = mat_light.clone();
            let mat_dark_row = mat_dark.clone();
            let mat_2d_light_row = mat_2d_light.clone();
            let mat_2d_dark_row = mat_2d_dark.clone();

            (0..8).map(move |file| {
                let square = Square::new(file, rank);
                let is_white_square = square.is_white();

                // 3D Material
                let base_mat_3d = if is_white_square {
                    mat_light_row.clone()
                } else {
                    mat_dark_row.clone()
                };
                
                // 2D Material
                let base_mat_2d = if is_white_square {
                    mat_2d_light_row.clone()
                } else {
                    mat_2d_dark_row.clone()
                };

                let file_char = (b'a' + file) as char;
                let rank_num = rank + 1;
                let square_name = format!("Square {}{}", file_char, rank_num);

                let world_pos = Vec3::new(file as f32, 0., rank as f32);

                (
                    Transform::from_translation(world_pos),
                    Visibility::default(),
                    InheritedVisibility::default(),
                    PointerInteraction::default(),
                    bevy::picking::Pickable::default(),
                    square,
                    Board,
                    Name::new(square_name),
                    DespawnOnExit(GameState::InGame),
                    mesh_3d.clone(),
                    base_mat_3d,
                    mesh_2d.clone(),
                    base_mat_2d,
                )
            })
        })
        .collect();

    for (transform, vis, inher_vis, ptr, pick, square, board, name, exit, m3d, mat3d, m2d, mat2d) in squares {
        commands
            .spawn((
                transform,
                vis,
                inher_vis,
                ptr,
                pick,
                square,
                board,
                name,
                exit,
            ))
            .observe(on_square_click)
            .observe(on_square_hover)
            .observe(on_square_unhover)
            .with_children(|parent| {
                // 3D Visual
                parent.spawn((
                    Mesh3d(m3d),
                    MeshMaterial3d(mat3d),
                    BoardSquare3DVisual,
                    bevy::picking::Pickable::default(),
                ));
                
                // 2D Visual
                parent.spawn((
                    Mesh3d(m2d),
                    MeshMaterial3d(mat2d),
                    Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
                    BoardSquare2DVisual,
                    Visibility::Hidden,
                ));
            });
    }
}

pub fn board_view_mode_toggle_system(
    view_mode: Res<ViewMode>,
    mut board_3d_query: Query<&mut Visibility, (With<BoardSquare3DVisual>, Without<BoardSquare2DVisual>)>,
    mut board_2d_query: Query<&mut Visibility, (With<BoardSquare2DVisual>, Without<BoardSquare3DVisual>)>,
) {
    let mode = *view_mode;
    let show_3d = mode == ViewMode::Standard3D || mode == ViewMode::TempleOS;
    let show_2d = mode == ViewMode::Standard2D;

    for mut vis in board_3d_query.iter_mut() {
        *vis = if show_3d { Visibility::Visible } else { Visibility::Hidden };
    }
    for mut vis in board_2d_query.iter_mut() {
        *vis = if show_2d { Visibility::Visible } else { Visibility::Hidden };
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
                board_view_mode_toggle_system.run_if(in_state(GameState::InGame)),
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
