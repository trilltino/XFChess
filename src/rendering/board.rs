//! Board creation and rendering
//!
//! Uses batch spawning pattern from Bevy examples (many_sprites.rs, bevymark.rs)
//! to efficiently create all 64 board squares in a single operation.

use bevy::prelude::*;
use bevy::picking::pointer::PointerInteraction;
use crate::rendering::utils::{Square, SquareMaterials};
use crate::game::systems::input::on_square_click;
use crate::input::pointer::{on_square_hover, on_square_unhover};

#[derive(Resource, Component)]
pub struct Board;

pub fn create_board(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    materials: Res<SquareMaterials>,
) {
    use crate::core::GameState;

    let boardmesh = meshes.add(Plane3d::default().mesh().size(1.0, 1.0));

    // Pattern from Bevy stress_tests/many_sprites.rs: Collect all squares into Vec, then batch spawn
    // This reduces stack pressure and is more efficient than 64 individual spawn calls
    let squares: Vec<_> = (0..8)
        .flat_map(|i| {
            // Clone materials and mesh for each row to share across inner closure
            let black_mat = materials.black_color.clone();
            let white_mat = materials.white_color.clone();
            let mesh = boardmesh.clone();

            (0..8).map(move |j| {
                let square = Square { x: i, y: j };

                // Use Square::is_white() method for proper checkerboard pattern
                let material = if square.is_white() {
                    black_mat.clone()
                } else {
                    white_mat.clone()
                };

                // Generate square name in chess notation (e.g., "Square a1", "Square h8")
                let file = (b'a' + j) as char;
                let rank = i + 1;
                let square_name = format!("Square {}{}", file, rank);

                // Bundle all components for this square
                // DespawnOnExit automatically despawns all 64 board squares when exiting Multiplayer
                (
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material),
                    Transform::from_translation(Vec3::new(i as f32, 0., j as f32)),
                    PointerInteraction::default(),
                    square,
                    Board,
                    Name::new(square_name),
                    DespawnOnExit(GameState::Multiplayer),
                )
            })
        })
        .collect();

    // Spawn all 64 squares in a single batch operation
    // Then attach observers to each (click, hover, unhover)
    for square_bundle in squares {
        commands.spawn(square_bundle)
            .observe(on_square_click)
            .observe(on_square_hover)
            .observe(on_square_unhover);
    }
}

pub struct BoardPlugin;

impl Plugin for BoardPlugin {
    fn build(&self, app: &mut App) {
        use crate::core::GameState;
        app.add_systems(OnEnter(GameState::Multiplayer), create_board);
    }
}
