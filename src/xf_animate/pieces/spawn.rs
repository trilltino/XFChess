use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;

use super::assets::MiniAssets;
use super::components::MiniPiece;
use crate::core::{DespawnOnExit, GameState};
use crate::rendering::pieces::{PieceColor, PieceType};
use crate::xf_animate::board::square_world;
use crate::xf_animate::viewport::MINI_LAYER;

pub const PIECE_MESH_SCALE: f32 = 0.95;

pub fn spawn_mini_piece(
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
    let rotation = piece_rotation(kind, color);

    commands
        .spawn((
            Transform::from_translation(pos).with_rotation(rotation),
            Visibility::Inherited,
            MiniPiece {
                file,
                rank,
                color,
                kind,
            },
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

fn piece_rotation(kind: PieceType, color: PieceColor) -> Quat {
    match (kind, color) {
        (PieceType::Knight, PieceColor::White) => {
            Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2)
        }
        (PieceType::Knight, PieceColor::Black) => {
            Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)
        }
        (_, PieceColor::White) => Quat::IDENTITY,
        (_, PieceColor::Black) => Quat::from_rotation_y(std::f32::consts::PI),
    }
}
