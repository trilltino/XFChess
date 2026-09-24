use bevy::prelude::*;

use super::assets::{load_mini_assets, MiniAssets};
use super::spawn::spawn_mini_piece;
use crate::rendering::pieces::{PieceColor, PieceType};

pub fn spawn_mini_pieces(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let assets = load_mini_assets(&asset_server, &mut materials);
    spawn_starting_position(&mut commands, &assets);
    commands.insert_resource(assets);
}

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
        spawn_mini_piece(
            commands,
            assets,
            PieceColor::White,
            PieceType::Pawn,
            file,
            1,
        );
    }
    for (file, &kind) in BACK_ROW.iter().enumerate() {
        spawn_mini_piece(commands, assets, PieceColor::Black, kind, file as u8, 7);
    }
    for file in 0..8u8 {
        spawn_mini_piece(
            commands,
            assets,
            PieceColor::Black,
            PieceType::Pawn,
            file,
            6,
        );
    }
}
