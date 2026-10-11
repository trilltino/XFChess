use super::*;

/// Ivory/cream piece with low roughness — specular highlights define the silhouette.
pub fn white_piece_material() -> StandardMaterial {
    StandardMaterial {
        base_color: Color::srgb(0.92, 0.89, 0.82), // warm ivory, not pure white
        perceptual_roughness: 0.25,
        metallic: 0.0,
        reflectance: 0.55,
        ..default()
    }
}

/// Dark charcoal piece — not pure black so light still picks out the edges.
pub fn black_piece_material() -> StandardMaterial {
    StandardMaterial {
        base_color: Color::srgb(0.10, 0.08, 0.07), // very dark warm brown-black
        perceptual_roughness: 0.20,
        metallic: 0.0,
        reflectance: 0.50,
        ..default()
    }
}

/// Mesh handles from wooden_chess_board.glb, indexed by piece type and color.
#[derive(Resource)]
pub struct PieceMeshes {
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

#[derive(Resource)]
pub struct PieceSpriteHandles {
    pub white_king: Handle<Image>,
    pub white_queen: Handle<Image>,
    pub white_rook: Handle<Image>,
    pub white_bishop: Handle<Image>,
    pub white_knight: Handle<Image>,
    pub white_pawn: Handle<Image>,
    pub black_king: Handle<Image>,
    pub black_queen: Handle<Image>,
    pub black_rook: Handle<Image>,
    pub black_bishop: Handle<Image>,
    pub black_knight: Handle<Image>,
    pub black_pawn: Handle<Image>,
}

impl PieceSpriteHandles {
    pub fn get(&self, piece_type: PieceType, color: PieceColor) -> Handle<Image> {
        match (piece_type, color) {
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

impl PieceMeshes {
    pub fn get(&self, piece_type: PieceType, color: PieceColor) -> Handle<Mesh> {
        match (piece_type, color) {
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

    pub fn all_ids(&self) -> [bevy::asset::AssetId<Mesh>; 12] {
        [
            self.white_king.id(),
            self.white_queen.id(),
            self.white_rook.id(),
            self.white_bishop.id(),
            self.white_knight.id(),
            self.white_pawn.id(),
            self.black_king.id(),
            self.black_queen.id(),
            self.black_rook.id(),
            self.black_bishop.id(),
            self.black_knight.id(),
            self.black_pawn.id(),
        ]
    }
}

pub(super) fn piece_set_folder(piece_set: u8) -> &'static str {
    match piece_set {
        1 => "alpha",
        2 => "merida",
        3 => "vecteezy",
        _ => "cburnett",
    }
}

pub(super) fn load_sprite_handles_for_set(
    asset_server: &AssetServer,
    piece_set: u8,
) -> PieceSpriteHandles {
    let folder = piece_set_folder(piece_set);
    PieceSpriteHandles {
        white_bishop: asset_server.load(format!("pieces/2d/{}/wb.png", folder)),
        white_king: asset_server.load(format!("pieces/2d/{}/wk.png", folder)),
        white_knight: asset_server.load(format!("pieces/2d/{}/wn.png", folder)),
        white_pawn: asset_server.load(format!("pieces/2d/{}/wp.png", folder)),
        white_queen: asset_server.load(format!("pieces/2d/{}/wq.png", folder)),
        white_rook: asset_server.load(format!("pieces/2d/{}/wr.png", folder)),
        black_bishop: asset_server.load(format!("pieces/2d/{}/bb.png", folder)),
        black_king: asset_server.load(format!("pieces/2d/{}/bk.png", folder)),
        black_knight: asset_server.load(format!("pieces/2d/{}/bn.png", folder)),
        black_pawn: asset_server.load(format!("pieces/2d/{}/bp.png", folder)),
        black_queen: asset_server.load(format!("pieces/2d/{}/bq.png", folder)),
        black_rook: asset_server.load(format!("pieces/2d/{}/br.png", folder)),
    }
}

/// Re-loads piece sprites whenever `GameSettings.piece_set` changes or on initial startup.
pub fn reload_piece_sprites(
    settings: Res<crate::core::GameSettings>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    let sprites = load_sprite_handles_for_set(&asset_server, settings.piece_set);
    commands.insert_resource(sprites);
}

pub(super) fn load_piece_meshes(mut commands: Commands, asset_server: Res<AssetServer>) {
    info!("[PIECES] Loading piece meshes from wooden_chess_board.glb");

    let meshes = PieceMeshes {
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
    };

    commands.insert_resource(meshes);

    info!("[PIECES] Loading 2D piece sprites from assets/pieces/2d/");
    let sprites = load_sprite_handles_for_set(&asset_server, 0);
    commands.insert_resource(sprites);

    info!("[PIECES] Mesh and Sprite handles created - waiting for assets to load");
}
