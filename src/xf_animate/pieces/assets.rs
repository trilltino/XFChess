use bevy::prelude::*;

use crate::rendering::pieces::{PieceColor, PieceType};

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
            white_bishop: asset_server.load("models/wooden_chess_board.glb#Mesh18/Primitive0"),
            white_king: asset_server.load("models/wooden_chess_board.glb#Mesh20/Primitive0"),
            white_knight: asset_server.load("models/wooden_chess_board.glb#Mesh21/Primitive0"),
            white_pawn: asset_server.load("models/wooden_chess_board.glb#Mesh23/Primitive0"),
            white_queen: asset_server.load("models/wooden_chess_board.glb#Mesh31/Primitive0"),
            white_rook: asset_server.load("models/wooden_chess_board.glb#Mesh32/Primitive0"),
            black_bishop: asset_server.load("models/wooden_chess_board.glb#Mesh0/Primitive0"),
            black_king: asset_server.load("models/wooden_chess_board.glb#Mesh2/Primitive0"),
            black_knight: asset_server.load("models/wooden_chess_board.glb#Mesh3/Primitive0"),
            black_pawn: asset_server.load("models/wooden_chess_board.glb#Mesh5/Primitive0"),
            black_queen: asset_server.load("models/wooden_chess_board.glb#Mesh13/Primitive0"),
            black_rook: asset_server.load("models/wooden_chess_board.glb#Mesh14/Primitive0"),
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

pub fn load_mini_assets(
    asset_server: &AssetServer,
    materials: &mut Assets<StandardMaterial>,
) -> MiniAssets {
    MiniAssets {
        meshes: MiniMeshes::load(asset_server),
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
    }
}
