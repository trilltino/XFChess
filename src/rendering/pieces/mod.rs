//! GLTF piece rendering. Re-export the game component types for compatibility.

use crate::game::components::HasMoved;
use crate::input::board::{on_piece_click, on_piece_drag, on_piece_drag_end, on_piece_drag_start};
use crate::input::pointer::{on_piece_hover, on_piece_unhover};
use bevy::color::Color;

use bevy::camera::visibility::RenderLayers;
use bevy::picking::pointer::PointerInteraction;
use bevy::prelude::*;
use std::f32;

// Re-export canonical component types for existing rendering::pieces imports.
pub use crate::game::components::piece_types::{Piece, PieceColor, PieceType};

mod assets;
mod spawn;
pub use assets::*;
pub use spawn::*;

/// Mesh-local Y offset; GLB origins are at the piece base.
const PIECE_Y_OFFSET: f32 = 0.0;

/// Scale factor for piece meshes — fits the chess kit models to board squares
/// Reference uses 1.0 scale for wooden_chess_board.glb
pub const PIECE_MESH_SCALE: f32 = 1.0;

/// Board-surface Y coordinate: half the 0.1-unit square thickness.
pub const PIECE_ON_BOARD_Y: f32 = 0.05;

/// Resource to track if pieces have been spawned for current game
#[derive(Resource, Default)]
pub struct PiecesSpawned {
    pub spawned: bool,
}

/// Component marking a 3D visual element of a piece
#[derive(Component)]
pub struct Piece3DVisual;

/// Component marking a 2D visual element of a piece
#[derive(Component)]
pub struct Piece2DVisual;

/// Resume position from a verified move log. Set before InGame; resume applies
/// the same FEN to the engine after pieces spawn, then clears it.
#[derive(Resource, Default, Debug, Clone)]
pub struct ResumeBoard {
    pub fen: Option<String>,
}

pub fn create_pieces(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    piece_meshes: Res<PieceMeshes>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    _view_mode: Res<crate::game::view_mode::ViewMode>,
    mut pieces_spawned: ResMut<PiecesSpawned>,
    sprite_handles: Option<Res<PieceSpriteHandles>>,
    puzzle_board: Option<Res<crate::puzzle::PuzzleBoard>>,
    resume_board: Option<Res<ResumeBoard>>,
) {
    // Skip if already spawned
    if pieces_spawned.spawned {
        return;
    }

    // Pieces are rendered in all modes to ensure full game state visibility

    // Check if all piece meshes are loaded
    let meshes_to_check = piece_meshes.all_ids();

    for mesh_id in meshes_to_check.iter() {
        match asset_server.load_state(*mesh_id) {
            bevy::asset::LoadState::Loaded => {}
            _ => {
                info!("[PIECES] Waiting for piece meshes to load...");
                return; // Not all meshes loaded yet, try again next frame
            }
        }
    }

    info!("[PIECES] All piece meshes loaded - spawning pieces");

    // Use the documented constant offset to position pieces on the board surface
    // See PIECE_Y_OFFSET documentation for how to recalculate if models change
    let visual_offset = Vec3::new(0.0, PIECE_Y_OFFSET, 0.0);

    // Puzzle mode: spawn the position described by the FEN instead of the
    // standard starting layout.
    if let Some(pb) = puzzle_board.as_ref() {
        if pb.active && !pb.fen.is_empty() {
            spawn_pieces_from_fen(
                &mut commands,
                &piece_meshes,
                &mut materials,
                &pb.fen,
                visual_offset,
                &sprite_handles,
            );
            pieces_spawned.spawned = true;
            info!("[PIECES] Spawned puzzle position from FEN");
            return;
        }
    }

    // Resumed online game: spawn the verified current position.
    if let Some(fen) = resume_board.as_ref().and_then(|rb| rb.fen.as_ref()) {
        spawn_pieces_from_fen(
            &mut commands,
            &piece_meshes,
            &mut materials,
            fen,
            visual_offset,
            &sprite_handles,
        );
        pieces_spawned.spawned = true;
        info!("[PIECES] Spawned resumed game position from FEN");
        return;
    }

    // Each piece will get its own unique material to prevent color bleeding
    // during capture animations. This ensures fade effects don't affect other pieces.

    // Data-driven piece placement using standard chess starting positions
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

    // Spawn white pieces (rank 0 in chess coordinates = rank 1 on board)
    for (file, &piece_type) in BACK_ROW.iter().enumerate() {
        let piece_material = materials.add(white_piece_material());
        spawn_piece_at(
            &mut commands,
            &piece_meshes,
            piece_material,
            PieceColor::White,
            piece_type,
            (file as u8, 0), // (file, rank) -> world (X, Z)
            visual_offset,
            &sprite_handles,
        );
    }

    // Spawn white pawns (rank 1 in chess coordinates = rank 2 on board)
    for file in 0..8 {
        let piece_material = materials.add(white_piece_material());
        spawn_piece_at(
            &mut commands,
            &piece_meshes,
            piece_material,
            PieceColor::White,
            PieceType::Pawn,
            (file as u8, 1), // (file, rank) -> world (X, Z)
            visual_offset,
            &sprite_handles,
        );
    }

    // Spawn black pieces (rank 7 in chess coordinates = rank 8 on board)
    for (file, &piece_type) in BACK_ROW.iter().enumerate() {
        let piece_material = materials.add(black_piece_material());
        spawn_piece_at(
            &mut commands,
            &piece_meshes,
            piece_material,
            PieceColor::Black,
            piece_type,
            (file as u8, 7), // (file, rank) -> world (X, Z)
            visual_offset,
            &sprite_handles,
        );
    }

    // Spawn black pawns (rank 6 in chess coordinates = rank 7 on board)
    for file in 0..8 {
        let piece_material = materials.add(black_piece_material());
        spawn_piece_at(
            &mut commands,
            &piece_meshes,
            piece_material,
            PieceColor::Black,
            PieceType::Pawn,
            (file as u8, 6), // (file, rank) -> world (X, Z)
            visual_offset,
            &sprite_handles,
        );
    }

    pieces_spawned.spawned = true;
    info!("[PIECES] All 32 pieces spawned successfully");
}

/// Preallocated picking-proxy assets for 2D top-down mode; 3D mode picks GLB geometry.
#[derive(Resource)]
pub struct PiecePickingAssets {
    pub mesh_2d: Handle<Mesh>,
    pub matl: Handle<StandardMaterial>,
}

/// Marker for the 2D picking proxy (flat slab, active only in 2D mode).
/// In 3D mode the actual Piece3DVisual mesh is used for picking instead.
#[derive(Component)]
pub struct PiecePickingProxy2D;

fn init_piece_picking_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    // Place the full-cell picking slab just above the board, matching the 2D sprite footprint.
    let mesh_2d = meshes.add(Cuboid::new(0.98, 0.02, 0.98));
    let matl = mats.add(StandardMaterial {
        base_color: Color::srgba(0.0, 0.0, 0.0, 0.0),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    commands.insert_resource(PiecePickingAssets { mesh_2d, matl });
}

/// Attach a flat picking proxy when Piece is added in 2D; sprites cannot use mesh picking.
fn on_piece_added(
    trigger: On<bevy::ecs::lifecycle::Add<Piece>>,
    mut commands: Commands,
    assets: Option<Res<PiecePickingAssets>>,
) {
    let Some(assets) = assets else { return };
    commands
        .entity(trigger.event_target())
        .with_children(|parent| {
            parent.spawn((
                Mesh3d(assets.mesh_2d.clone()),
                MeshMaterial3d(assets.matl.clone()),
                Transform::from_xyz(0.0, 0.06, 0.0),
                PiecePickingProxy2D,
                bevy::picking::Pickable::IGNORE, // activated by view_mode_rendering_toggle_system
                RenderLayers::layer(crate::game::systems::camera::BOARD_LAYER),
                Name::new("Piece Picking Proxy 2D"),
            ));
        });
}

pub struct PiecePlugin;
impl Plugin for PiecePlugin {
    fn build(&self, app: &mut App) {
        use crate::core::GameState;
        app.init_resource::<PiecesSpawned>();
        app.init_resource::<ResumeBoard>();
        app.add_systems(Startup, (load_piece_meshes, init_piece_picking_assets));
        app.add_systems(Update, create_pieces
                .after(crate::game::systems::camera::view_mode_toggle_input_system)
                .in_set(crate::core::schedule::GameSystems::Input));
        app.add_systems(OnExit(GameState::InGame), reset_pieces_spawned);
        // Apply ViewMode on entry, mode changes, and piece spawning.
        app.add_systems(
            OnEnter(GameState::InGame),
            view_mode_rendering_toggle_system,
        );
        app.add_systems(
            Update,
            view_mode_rendering_toggle_system
                .before(crate::rendering::board::board::board_view_mode_toggle_system)
                .in_set(crate::core::schedule::GameSystems::Visual)
                .run_if(
                    resource_changed::<crate::game::view_mode::ViewMode>
                        .or_else(resource_changed::<PiecesSpawned>),
                ),
        );
        app.add_observer(on_piece_added);
    }
}

pub fn view_mode_rendering_toggle_system(
    view_mode: Res<crate::game::view_mode::ViewMode>,
    mut piece_3d_query: Query<
        (&mut Visibility, &mut bevy::picking::Pickable),
        (With<Piece3DVisual>, Without<Piece2DVisual>),
    >,
    mut piece_2d_query: Query<&mut Visibility, (With<Piece2DVisual>, Without<Piece3DVisual>)>,
    mut proxy_2d_query: Query<
        &mut bevy::picking::Pickable,
        (With<PiecePickingProxy2D>, Without<Piece3DVisual>),
    >,
) {
    let mode = *view_mode;
    let (show_3d, show_2d) = match mode {
        crate::game::view_mode::ViewMode::Standard3D => (true, false),
        crate::game::view_mode::ViewMode::Standard2D => (false, false), // Pure egui 2D board view; hide all 3D world entities
        #[cfg(feature = "templeos")]
        crate::game::view_mode::ViewMode::TempleOS => (true, false),
    };

    for (mut vis, mut pick) in piece_3d_query.iter_mut() {
        *vis = if show_3d {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        // In 2D mode disable 3D mesh picking so only the flat 2D proxy receives clicks.
        *pick = if show_3d {
            bevy::picking::Pickable::default()
        } else {
            bevy::picking::Pickable::IGNORE
        };
    }
    for mut vis in piece_2d_query.iter_mut() {
        *vis = if show_2d {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    for mut pick in proxy_2d_query.iter_mut() {
        *pick = if show_2d {
            bevy::picking::Pickable::default()
        } else {
            bevy::picking::Pickable::IGNORE
        };
    }
}

fn reset_pieces_spawned(mut pieces_spawned: ResMut<PiecesSpawned>) {
    pieces_spawned.spawned = false;
    info!("[PIECES] Reset spawn flag for next game");
}
