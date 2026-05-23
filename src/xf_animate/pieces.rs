//! Mini chess pieces rendered on [`MINI_LAYER`] using the same GLTF meshes as
//! the in-game board.

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;

use super::board::square_world;
use super::viewport::MINI_LAYER;
use crate::core::{DespawnOnExit, GameState};
use crate::rendering::pieces::{PieceColor, PieceType};

/// Visual scale applied to the piece child mesh (board squares are 1.0 units).
pub const PIECE_MESH_SCALE: f32 = 0.95;

/// Component tracking a mini piece's logical board position.
#[derive(Component, Debug)]
pub struct MiniPiece {
    pub file: u8,
    pub rank: u8,
    pub color: PieceColor,
    pub kind: PieceType,
}

/// Cached asset handles + materials for restarting the loop without re-loading.
#[derive(Resource)]
pub struct MiniAssets {
    pub meshes: MiniMeshes,
    pub white_mat: Handle<StandardMaterial>,
    pub black_mat: Handle<StandardMaterial>,
}

pub struct MiniMeshes {
    pub white_king: Handle<Mesh>,
    pub white_queen: Handle<Mesh>,
    pub white_rook: Handle<Mesh>,
    pub white_bishop: Handle<Mesh>,
    pub white_knight: Handle<Mesh>,
    pub white_pawn: Handle<Mesh>,
    pub black_king: Handle<Mesh>,
    pub black_queen: Handle<Mesh>,
    pub black_rook: Handle<Mesh>,
    pub black_bishop: Handle<Mesh>,
    pub black_knight: Handle<Mesh>,
    pub black_pawn: Handle<Mesh>,
}

impl MiniMeshes {
    pub fn load(asset_server: &AssetServer) -> Self {
        Self {
            white_bishop: asset_server
                .load("models/wooden_chess_board.glb#Mesh18/Primitive0"),
            white_king: asset_server
                .load("models/wooden_chess_board.glb#Mesh20/Primitive0"),
            white_knight: asset_server
                .load("models/wooden_chess_board.glb#Mesh21/Primitive0"),
            white_pawn: asset_server
                .load("models/wooden_chess_board.glb#Mesh23/Primitive0"),
            white_queen: asset_server
                .load("models/wooden_chess_board.glb#Mesh31/Primitive0"),
            white_rook: asset_server
                .load("models/wooden_chess_board.glb#Mesh32/Primitive0"),
            black_bishop: asset_server
                .load("models/wooden_chess_board.glb#Mesh0/Primitive0"),
            black_king: asset_server
                .load("models/wooden_chess_board.glb#Mesh2/Primitive0"),
            black_knight: asset_server
                .load("models/wooden_chess_board.glb#Mesh3/Primitive0"),
            black_pawn: asset_server
                .load("models/wooden_chess_board.glb#Mesh5/Primitive0"),
            black_queen: asset_server
                .load("models/wooden_chess_board.glb#Mesh13/Primitive0"),
            black_rook: asset_server
                .load("models/wooden_chess_board.glb#Mesh14/Primitive0"),
        }
    }

    pub fn get(&self, kind: PieceType, color: PieceColor) -> Handle<Mesh> {
        match (kind, color) {
            (PieceType::King, PieceColor::White) => self.white_king.clone(),
            (PieceType::Queen, PieceColor::White) => self.white_queen.clone(),
            (PieceType::Rook, PieceColor::White) => self.white_rook.clone(),
            (PieceType::Bishop, PieceColor::White) => self.white_bishop.clone(),
            (PieceType::Knight, PieceColor::White) => self.white_knight.clone(),
            (PieceType::Pawn, PieceColor::White) => self.white_pawn.clone(),
            (PieceType::King, PieceColor::Black) => self.black_king.clone(),
            (PieceType::Queen, PieceColor::Black) => self.black_queen.clone(),
            (PieceType::Rook, PieceColor::Black) => self.black_rook.clone(),
            (PieceType::Bishop, PieceColor::Black) => self.black_bishop.clone(),
            (PieceType::Knight, PieceColor::Black) => self.black_knight.clone(),
            (PieceType::Pawn, PieceColor::Black) => self.black_pawn.clone(),
        }
    }
}

/// One-shot: load meshes, create materials, spawn the starting position, and
/// cache the assets resource for restart cycles.
pub fn spawn_mini_pieces(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let assets = MiniAssets {
        meshes: MiniMeshes::load(&asset_server),
        white_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(0.96, 0.94, 0.88),
            perceptual_roughness: 0.55,
            ..default()
        }),
        black_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(0.12, 0.12, 0.12),
            perceptual_roughness: 0.55,
            ..default()
        }),
    };
    spawn_starting_position(&mut commands, &assets);
    commands.insert_resource(assets);
}

/// Spawn all 32 pieces in their starting positions.
pub fn spawn_starting_position(commands: &mut Commands, assets: &MiniAssets) {
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

    for (file, &kind) in BACK_ROW.iter().enumerate() {
        spawn_mini_piece(commands, assets, PieceColor::White, kind, file as u8, 0);
    }
    for file in 0..8u8 {
        spawn_mini_piece(commands, assets, PieceColor::White, PieceType::Pawn, file, 1);
    }
    for (file, &kind) in BACK_ROW.iter().enumerate() {
        spawn_mini_piece(commands, assets, PieceColor::Black, kind, file as u8, 7);
    }
    for file in 0..8u8 {
        spawn_mini_piece(commands, assets, PieceColor::Black, PieceType::Pawn, file, 6);
    }
}

fn knight_rotation(color: PieceColor) -> Quat {
    match color {
        PieceColor::White => Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2),
        PieceColor::Black => Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
    }
}

fn spawn_mini_piece(
    commands: &mut Commands,
    assets: &MiniAssets,
    color: PieceColor,
    kind: PieceType,
    file: u8,
    rank: u8,
) {
    let pos = square_world(file, rank);
    let material = match color {
        PieceColor::White => assets.white_mat.clone(),
        PieceColor::Black => assets.black_mat.clone(),
    };
    let mesh = assets.meshes.get(kind, color);

    // Mirror the in-game orientation: black pieces rotate 180-¦ so they face white.
    let rotation = match (kind, color) {
        (PieceType::Knight, _) => knight_rotation(color),
        (_, PieceColor::White) => Quat::IDENTITY,
        (_, PieceColor::Black) => Quat::from_rotation_y(std::f32::consts::PI),
    };

    commands
        .spawn((
            Transform::from_translation(pos).with_rotation(rotation),
            Visibility::Inherited,
            MiniPiece { file, rank, color, kind },
            RenderLayers::layer(MINI_LAYER),
            DespawnOnExit(GameState::MainMenu),
            Name::new(format!("Mini {:?} {:?}", color, kind)),
        ))
        .with_children(|parent| {
            parent.spawn((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Transform::from_scale(Vec3::splat(PIECE_MESH_SCALE)),
                RenderLayers::layer(MINI_LAYER),
            ));
        });
}
