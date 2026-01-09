//! Showcase chess game on pyramid top in main menu
//!
//! Spawns a miniature chess board with real GLTF pieces on the pyramid top
//! and plays an automated AI vs AI game in the background.

use crate::core::{DespawnOnExit, GameState};
use crate::rendering::pieces::{Piece, PieceColor, PieceType};
use bevy::prelude::*;

/// Scale factor for showcase (pyramid top layer is ~6 units, board is 8 squares)
const SHOWCASE_SCALE: f32 = 0.70;
/// Height of showcase board above pyramid top  
const SHOWCASE_Y: f32 = 0.55;
/// Offset to center the board on pyramid (-3.5 squares * scale)
const SHOWCASE_OFFSET: f32 = -2.45;
/// The game uses 0.2 scale for pieces
const GAME_MESH_SCALE: f32 = 0.2;
/// Piece mesh scale for showcase (smaller than game)
const PIECE_MESH_SCALE: f32 = 0.12;
/// Ratio to scale offsets from game to showcase
const OFFSET_RATIO: f32 = PIECE_MESH_SCALE / GAME_MESH_SCALE; // 0.6

/// Marker for showcase board squares
#[derive(Component)]
pub struct ShowcaseSquare;

/// Marker for showcase pieces (not interactive)
#[derive(Component)]
pub struct ShowcasePiece {
    pub x: u8,
    pub y: u8,
}

/// Animation for showcase piece movement
#[derive(Component)]
pub struct ShowcaseMoveAnimation {
    pub start: Vec3,
    pub end: Vec3,
    pub elapsed: f32,
    pub duration: f32,
}

/// Fade animation for captured showcase pieces
#[derive(Component)]
pub struct ShowcaseFadeOut {
    pub timer: Timer,
}

/// Resource tracking showcase game state
#[derive(Resource)]
pub struct ShowcaseGameState {
    pub move_timer: Timer,
    pub move_index: usize,
    pub game_over: bool,
}

impl Default for ShowcaseGameState {
    fn default() -> Self {
        Self {
            move_timer: Timer::from_seconds(3.5, TimerMode::Repeating), // Slower moves
            move_index: 0,
            game_over: false,
        }
    }
}

/// 40 moves from a real game (Italian Game / Giuoco Piano)
const SHOWCASE_MOVES: [(u8, u8, u8, u8, bool); 40] = [
    // (from_x, from_y, to_x, to_y, is_capture)
    (1, 4, 3, 4, false), // 1. e4
    (6, 4, 4, 4, false), // 1... e5
    (0, 6, 2, 5, false), // 2. Nf3
    (7, 1, 5, 2, false), // 2... Nc6
    (0, 5, 3, 2, false), // 3. Bc4
    (7, 5, 4, 2, false), // 3... Bc5
    (1, 2, 2, 2, false), // 4. c3
    (6, 6, 5, 6, false), // 4... Nf6
    (1, 3, 3, 3, false), // 5. d4
    (4, 4, 3, 3, true),  // 5... exd4
    (2, 2, 3, 3, true),  // 6. cxd4
    (4, 2, 5, 1, false), // 6... Bb4+
    (0, 1, 2, 2, false), // 7. Nc3
    (5, 6, 3, 4, true),  // 7... Nxe4
    (0, 0, 0, 0, false), // 8. O-O (simplified - no castle)
    (3, 4, 2, 2, true),  // 8... Nxc3
    (1, 1, 2, 2, true),  // 9. bxc3
    (5, 1, 2, 4, false), // 9... Bxc3?
    (0, 3, 1, 4, false), // 10. Qb3
    (2, 4, 0, 2, true),  // 10... Bxa1
    (0, 5, 6, 5, false), // 11. Bxf7+
    (7, 4, 7, 5, false), // 11... Kf8 (simplified)
    (1, 4, 5, 4, false), // 12. Qe3+ (check)
    (6, 3, 5, 3, false), // 12... d6
    (6, 5, 5, 4, false), // 13. Bxe4? (error but keeps game going)
    (5, 2, 4, 3, false), // 13... Nd4
    (2, 5, 4, 4, false), // 14. Nxd4
    (0, 2, 1, 3, false), // 14... Qxd4
    (5, 4, 6, 3, true),  // 15. Bxd6+
    (7, 5, 6, 5, false), // 15... Kg8
    (6, 3, 4, 5, false), // 16. Be7
    (6, 2, 5, 2, false), // 16... c6
    (4, 5, 5, 6, false), // 17. Bf6
    (1, 3, 3, 5, false), // 17... Qf4
    (5, 6, 6, 7, true),  // 18. Bxg7
    (7, 7, 6, 7, true),  // 18... Rxg7
    (3, 5, 6, 5, false), // 19. Qf6
    (6, 7, 5, 7, false), // 19... Rg7
    (6, 5, 7, 6, false), // 20. Qh8# (checkmate)
    (5, 7, 6, 7, false), // game ends
];

/// Convert showcase board position to world position
fn showcase_world_pos(x: u8, y: u8) -> Vec3 {
    Vec3::new(
        x as f32 * SHOWCASE_SCALE + SHOWCASE_OFFSET,
        SHOWCASE_Y,
        y as f32 * SHOWCASE_SCALE + SHOWCASE_OFFSET,
    )
}

/// Spawn showcase chess board on pyramid top
pub fn spawn_showcase_board(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let square_size = SHOWCASE_SCALE;
    let mesh = meshes.add(Cuboid::new(square_size, 0.015, square_size));

    let light_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.92, 0.88, 0.78),
        ..default()
    });
    let dark_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.55, 0.35, 0.20),
        ..default()
    });

    for x in 0..8u8 {
        for y in 0..8u8 {
            let is_white = (x + y) % 2 == 0;
            let material = if is_white {
                light_mat.clone()
            } else {
                dark_mat.clone()
            };
            let pos = showcase_world_pos(x, y);

            commands.spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material),
                Transform::from_translation(pos),
                ShowcaseSquare,
                DespawnOnExit(GameState::MainMenu),
                Name::new(format!("ShowcaseSquare {}{}", (b'a' + y) as char, x + 1)),
            ));
        }
    }

    info!("[SHOWCASE] Spawned showcase board on pyramid");
}

/// Spawn showcase pieces using GLTF assets
pub fn spawn_showcase_pieces(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Load piece meshes from GLTF
    let meshes = ShowcaseMeshes {
        king: asset_server.load("models/chess_kit/pieces.glb#Mesh0/Primitive0"),
        king_cross: asset_server.load("models/chess_kit/pieces.glb#Mesh1/Primitive0"),
        pawn: asset_server.load("models/chess_kit/pieces.glb#Mesh2/Primitive0"),
        knight_1: asset_server.load("models/chess_kit/pieces.glb#Mesh3/Primitive0"),
        knight_2: asset_server.load("models/chess_kit/pieces.glb#Mesh4/Primitive0"),
        rook: asset_server.load("models/chess_kit/pieces.glb#Mesh5/Primitive0"),
        bishop: asset_server.load("models/chess_kit/pieces.glb#Mesh6/Primitive0"),
        queen: asset_server.load("models/chess_kit/pieces.glb#Mesh7/Primitive0"),
    };

    let white_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.95, 0.90),
        ..default()
    });
    let black_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.15, 0.12, 0.10),
        ..default()
    });

    const BACK_ROW: [PieceType; 8] = [
        PieceType::Rook,
        PieceType::Knight,
        PieceType::Bishop,
        PieceType::Queen,
        PieceType::King,
        PieceType::Bishop,
        PieceType::Knight,
        PieceType::Rook,
    ];

    // White pieces (rank 0)
    for (y, &piece_type) in BACK_ROW.iter().enumerate() {
        spawn_showcase_piece(
            &mut commands,
            &meshes,
            &white_mat,
            PieceColor::White,
            piece_type,
            0,
            y as u8,
        );
    }
    for y in 0..8u8 {
        spawn_showcase_piece(
            &mut commands,
            &meshes,
            &white_mat,
            PieceColor::White,
            PieceType::Pawn,
            1,
            y,
        );
    }

    // Black pieces (rank 7)
    for (y, &piece_type) in BACK_ROW.iter().enumerate() {
        spawn_showcase_piece(
            &mut commands,
            &meshes,
            &black_mat,
            PieceColor::Black,
            piece_type,
            7,
            y as u8,
        );
    }
    for y in 0..8u8 {
        spawn_showcase_piece(
            &mut commands,
            &meshes,
            &black_mat,
            PieceColor::Black,
            PieceType::Pawn,
            6,
            y,
        );
    }

    info!("[SHOWCASE] Spawned 32 showcase pieces with GLTF models");
}

struct ShowcaseMeshes {
    king: Handle<Mesh>,
    king_cross: Handle<Mesh>,
    pawn: Handle<Mesh>,
    knight_1: Handle<Mesh>,
    knight_2: Handle<Mesh>,
    rook: Handle<Mesh>,
    bishop: Handle<Mesh>,
    queen: Handle<Mesh>,
}

fn spawn_showcase_piece(
    commands: &mut Commands,
    meshes: &ShowcaseMeshes,
    material: &Handle<StandardMaterial>,
    color: PieceColor,
    piece_type: PieceType,
    x: u8,
    y: u8,
) {
    let pos = showcase_world_pos(x, y);

    let rotation = match color {
        PieceColor::White => Quat::IDENTITY,
        PieceColor::Black => Quat::from_rotation_y(std::f32::consts::PI),
    };

    // Get mesh and offset based on piece type (from pieces.rs)
    // These offsets are the raw GLTF mesh offsets, applied at mesh scale
    let (mesh, offset) = match piece_type {
        PieceType::King => (meshes.king.clone(), Vec3::new(-0.2, 0., -1.9)),
        PieceType::Queen => (meshes.queen.clone(), Vec3::new(-0.2, 0., -0.95)),
        PieceType::Bishop => (meshes.bishop.clone(), Vec3::new(-0.1, 0., 0.0)),
        PieceType::Knight => (meshes.knight_1.clone(), Vec3::new(-0.2, 0., 0.9)),
        PieceType::Rook => (meshes.rook.clone(), Vec3::new(-0.1, 0., 1.8)),
        PieceType::Pawn => (meshes.pawn.clone(), Vec3::new(-0.2, 0., 2.6)),
    };

    commands
        .spawn((
            Transform::from_translation(pos).with_rotation(rotation),
            Visibility::Inherited,
            Piece {
                color,
                piece_type,
                x,
                y,
            },
            ShowcasePiece { x, y },
            DespawnOnExit(GameState::MainMenu),
            Name::new(format!("Showcase {:?} {:?}", color, piece_type)),
        ))
        .with_children(|parent| {
            // Scale offset by ratio since game uses 0.2 scale, we use 0.12
            // This keeps pieces centered on their squares
            parent.spawn((
                Mesh3d(mesh),
                MeshMaterial3d(material.clone()),
                Transform::from_translation(offset * OFFSET_RATIO)
                    .with_scale(Vec3::splat(PIECE_MESH_SCALE)),
            ));
        });
}

/// System to autoplay showcase game with captures
pub fn run_showcase_game(
    time: Res<Time>,
    mut game_state: ResMut<ShowcaseGameState>,
    mut commands: Commands,
    mut pieces: Query<(Entity, &mut ShowcasePiece, &Transform), Without<ShowcaseMoveAnimation>>,
) {
    if game_state.game_over || game_state.move_index >= SHOWCASE_MOVES.len() {
        return;
    }

    game_state.move_timer.tick(time.delta());

    if game_state.move_timer.just_finished() {
        let (from_x, from_y, to_x, to_y, is_capture) = SHOWCASE_MOVES[game_state.move_index];

        // Skip castle placeholder moves
        if from_x == 0 && from_y == 0 && to_x == 0 && to_y == 0 {
            game_state.move_index += 1;
            return;
        }

        // If capture, find and fade out piece at destination
        if is_capture {
            for (entity, piece, _) in pieces.iter() {
                if piece.x == to_x && piece.y == to_y {
                    commands.entity(entity).insert(ShowcaseFadeOut {
                        timer: Timer::from_seconds(0.4, TimerMode::Once),
                    });
                    break;
                }
            }
        }

        // Find and animate moving piece
        for (entity, mut showcase_piece, transform) in pieces.iter_mut() {
            if showcase_piece.x == from_x && showcase_piece.y == from_y {
                let start = transform.translation;
                let mut end = showcase_world_pos(to_x, to_y);
                end.y = start.y;

                commands.entity(entity).insert(ShowcaseMoveAnimation {
                    start,
                    end,
                    elapsed: 0.0,
                    duration: 1.2, // Slower movement
                });

                showcase_piece.x = to_x;
                showcase_piece.y = to_y;

                game_state.move_index += 1;
                info!(
                    "[SHOWCASE] Move {}: ({},{}) -> ({},{}) capture={}",
                    game_state.move_index, from_x, from_y, to_x, to_y, is_capture
                );
                break;
            }
        }
    }
}

/// Animate showcase piece movement
pub fn animate_showcase_pieces(
    time: Res<Time>,
    mut commands: Commands,
    mut query: Query<(Entity, &mut Transform, &mut ShowcaseMoveAnimation)>,
) {
    for (entity, mut transform, mut anim) in query.iter_mut() {
        anim.elapsed += time.delta_secs();
        let progress = (anim.elapsed / anim.duration).min(1.0);

        // Smooth ease-in-out
        let t = progress * progress * (3.0 - 2.0 * progress);
        transform.translation = anim.start.lerp(anim.end, t);

        if progress >= 1.0 {
            commands.entity(entity).remove::<ShowcaseMoveAnimation>();
        }
    }
}

/// Fade out and despawn captured showcase pieces (using scale shrink)
pub fn animate_showcase_captures(
    time: Res<Time>,
    mut commands: Commands,
    mut query: Query<(Entity, &mut ShowcaseFadeOut, &mut Transform)>,
) {
    for (entity, mut fade, mut transform) in query.iter_mut() {
        fade.timer.tick(time.delta());
        let progress = fade.timer.fraction();

        // Shrink piece as it fades
        let scale = 1.0 - progress;
        transform.scale = Vec3::splat(scale.max(0.01));

        if fade.timer.just_finished() {
            commands.entity(entity).despawn();
        }
    }
}
