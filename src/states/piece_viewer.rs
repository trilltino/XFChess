//! Piece Viewer plugin - 3D model viewer for chess pieces
//!
//! Provides a viewer interface for inspecting chess pieces in 3D, similar to viewing
//! a weapon in an FPS game. Click a piece to open the viewer, then rotate it with the mouse.

use crate::core::{DespawnOnExit, MenuState};
use crate::rendering::pieces::{Piece, PieceColor, PieceType};
use crate::ui::styles::*;
use bevy::color::Color;

use bevy::input::mouse::{AccumulatedMouseMotion, MouseWheel};
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};

/// Plugin for piece viewer state
pub struct PieceViewerPlugin;

impl Plugin for PieceViewerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PieceViewerState>()
            .init_resource::<SelectedPieceInfo>()
            .add_systems(
                OnEnter(MenuState::PieceViewer),
                (
                    // Removed despawn_all_on_enter - now reuses Main Menu scene
                    setup_piece_viewer_camera,
                    setup_piece_viewer_scene,
                ),
            )
            .add_systems(OnExit(MenuState::PieceViewer), cleanup_piece_viewer)
            .add_systems(
                EguiPrimaryContextPass,
                piece_viewer_ui_wrapper.run_if(in_state(MenuState::PieceViewer)),
            )
            .add_systems(
                Update,
                (
                    update_piece_materials,
                    orbit_camera_system,
                    update_viewer_model,
                )
                    .run_if(in_state(MenuState::PieceViewer)),
            );
    }
}

/// Resource storing which piece was selected for viewing
#[derive(Resource, Default)]
pub struct SelectedPieceInfo {
    pub piece_type: Option<PieceType>,
    pub piece_color: Option<PieceColor>,
}

/// Observer function to open piece viewer when a piece is right-clicked
///
/// This can be attached to pieces in the menu or game to open the viewer.
/// Uses right-click (Secondary button) to distinguish from game piece selection (left-click).
pub fn on_piece_viewer_click(
    click: On<Pointer<Click>>,
    piece_query: Query<&Piece>,
    mut selected_piece: ResMut<SelectedPieceInfo>,
    mut menu_state: ResMut<NextState<MenuState>>,
    game_state: Res<State<crate::core::GameState>>,
) {
    // Only open viewer on right-click (Secondary button)
    // Left-click (Primary) is used for game piece selection
    if click.event.button != PointerButton::Secondary {
        return;
    }

    // Get piece info
    if let Ok(piece) = piece_query.get(click.entity) {
        selected_piece.piece_type = Some(piece.piece_type);
        selected_piece.piece_color = Some(piece.color);

        // Transition to viewer based on current game state
        match *game_state.get() {
            crate::core::GameState::MainMenu => {
                // In menu: set MenuState to PieceViewer (substate)
                menu_state.set(MenuState::PieceViewer);
                info!(
                    "[PIECE_VIEWER] Opening viewer for {:?} {:?}",
                    piece.color, piece.piece_type
                );
            }
            _ => {
                // In game: could pause and show viewer, but for now viewer is menu-only
                // This allows right-clicking pieces without opening viewer during gameplay
                info!("[PIECE_VIEWER] Right-clicked piece in game state (viewer available from menu via 'Piece Viewer' button)");
            }
        }
    }
}

/// Marker component for viewer camera
#[derive(Component)]
struct ViewerCamera;

/// Component for orbit camera controls
#[derive(Component)]
struct PieceViewerOrbitCamera {
    pub distance: f32,
    pub pitch: f32,
    pub yaw: f32,
    pub initialized: bool,
    pub focus: Vec3, // Focus point (pyramid top)
}

impl Default for PieceViewerOrbitCamera {
    fn default() -> Self {
        Self {
            distance: 8.0, // Back up a bit to see pyramid
            pitch: 0.3,
            yaw: 0.0,
            initialized: false,
            focus: Vec3::new(0.0, 0.5, 0.0), // Pyramid top is at Y=0
        }
    }
}

/// Enum for different viewable model types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewerModelType {
    #[default]
    ChessPiece,
    Human,
}

/// Resource tracking selected piece and material values
#[derive(Resource, Default)]
pub struct PieceViewerState {
    /// Selected piece entity (for material updates)
    pub selected_entity: Option<Entity>,

    /// Current model type being viewed
    pub model_type: ViewerModelType,

    // Material properties
    pub base_color: [f32; 3], // RGB
    pub metallic: f32,
    pub perceptual_roughness: f32,
    pub emissive: [f32; 4], // RGBA
    pub reflectance: f32,

    // Default values (for reset)
    pub default_base_color: [f32; 3],
    pub default_metallic: f32,
    pub default_perceptual_roughness: f32,
    pub default_emissive: [f32; 4],
    pub default_reflectance: f32,
}

impl PieceViewerState {
    fn reset_to_defaults(&mut self) {
        self.base_color = self.default_base_color;
        self.metallic = self.default_metallic;
        self.perceptual_roughness = self.default_perceptual_roughness;
        self.emissive = self.default_emissive;
        self.reflectance = self.default_reflectance;
    }
}

/// Despawn all entities except camera and global background when entering viewer
fn despawn_all_on_enter(
    mut commands: Commands,
    persistent_camera: Res<crate::PersistentEguiCamera>,
    all_entities: Query<(Entity, Option<&Name>), Without<ViewerCamera>>,
) {
    info!("[PIECE_VIEWER] Despawning all entities on enter");

    let mut despawned_count = 0;
    let mut skipped_count = 0;

    for (entity, name) in all_entities.iter() {
        // Skip persistent camera (has PrimaryEguiContext)
        if Some(entity) == persistent_camera.entity {
            skipped_count += 1;
            continue;
        }

        // Skip global background (persists across all states)
        if let Some(entity_name) = name {
            if entity_name.as_str() == "Global Background" {
                skipped_count += 1;
                continue;
            }
            // Skip other UI-related entities that should persist
            if entity_name.as_str() == "Persistent Egui Camera" {
                skipped_count += 1;
                continue;
            }
        }

        // Despawn everything else (pieces, board, lights, etc.)
        commands.entity(entity).despawn();
        despawned_count += 1;
    }

    info!(
        "[PIECE_VIEWER] Despawned {} entities, skipped {} (camera + global background)",
        despawned_count, skipped_count
    );
}

/// Setup orbit camera for piece viewer
fn setup_piece_viewer_camera(
    persistent_camera: Res<crate::PersistentEguiCamera>,
    mut camera_query: Query<
        &mut Transform,
        (With<bevy_egui::PrimaryEguiContext>, Without<ViewerCamera>),
    >,
    mut commands: Commands,
) {
    let camera_entity = match persistent_camera.entity {
        Some(entity) => entity,
        None => {
            error!("[PIECE_VIEWER] ERROR: Persistent camera entity is None");
            warn!("[PIECE_VIEWER] Camera setup will be skipped");
            return;
        }
    };

    let mut transform = match camera_query.get_mut(camera_entity) {
        Ok(transform) => transform,
        Err(e) => {
            error!(
                "[PIECE_VIEWER] ERROR: Failed to get camera transform: {:?}",
                e
            );
            warn!("[PIECE_VIEWER] Camera setup will be skipped");
            return;
        }
    };

    // Initial orbit position looking at pyramid top (where piece will be)
    let orbit = PieceViewerOrbitCamera::default();
    let pitch = orbit.pitch;
    let yaw = orbit.yaw;
    let distance = orbit.distance;
    let focus = orbit.focus;

    // Calculate camera position from orbit parameters around the focus point
    let x = focus.x + distance * pitch.cos() * yaw.sin();
    let y = focus.y + distance * pitch.sin();
    let z = focus.z + distance * pitch.cos() * yaw.cos();

    *transform = Transform::from_xyz(x, y, z).looking_at(focus, Vec3::Y);

    commands.entity(camera_entity).insert((ViewerCamera, orbit));

    info!("[PIECE_VIEWER] Camera set up for orbit viewing");
}

/// Orbit camera system - rotate around piece with mouse drag, zoom with scroll, and pan with WASD
fn orbit_camera_system(
    time: Res<Time>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut scroll_events: EventReader<MouseWheel>,
    mut query: Query<(&mut Transform, &mut PieceViewerOrbitCamera), With<ViewerCamera>>,
) {
    for (mut transform, mut orbit) in query.iter_mut() {
        // Initialize pitch/yaw from Transform on first frame
        if !orbit.initialized {
            let pos = transform.translation - orbit.focus;
            orbit.distance = pos.length();

            // Calculate pitch and yaw from position (spherical coordinates)
            // Pitch: angle from horizontal plane (elevation)
            orbit.pitch = (pos.y / orbit.distance).asin();
            // Yaw: angle around Y axis (azimuth)
            orbit.yaw = pos.z.atan2(pos.x);
            orbit.initialized = true;
        }

        let mut needs_update = false;

        // Zoom with scroll wheel
        for event in scroll_events.read() {
            const ZOOM_SENSITIVITY: f32 = 0.5;
            orbit.distance = (orbit.distance - event.y * ZOOM_SENSITIVITY).clamp(1.0, 50.0);
            needs_update = true;
        }

        // Keyboard Rotation (Q/E)
        const KEYBOARD_ROTATION_SPEED: f32 = 2.0;
        if keyboard_input.pressed(KeyCode::KeyQ) {
            // Rotate Left = Increase Yaw
            orbit.yaw += KEYBOARD_ROTATION_SPEED * time.delta_secs();
            needs_update = true;
        }
        if keyboard_input.pressed(KeyCode::KeyE) {
            // Rotate Right = Decrease Yaw
            orbit.yaw -= KEYBOARD_ROTATION_SPEED * time.delta_secs();
            needs_update = true;
        }

        // WASD Panning
        let mut pan = Vec3::ZERO;
        if keyboard_input.pressed(KeyCode::KeyW) {
            pan.z -= 1.0;
        }
        if keyboard_input.pressed(KeyCode::KeyS) {
            pan.z += 1.0;
        }
        if keyboard_input.pressed(KeyCode::KeyA) {
            pan.x -= 1.0;
        }
        if keyboard_input.pressed(KeyCode::KeyD) {
            pan.x += 1.0;
        }

        if pan != Vec3::ZERO {
            pan = pan.normalize() * 0.1; // Pan speed

            // Rotate pan vector by camera yaw to be relative to view
            // let rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2 - orbit.yaw);
            // Note: orbit.yaw of 0 implies camera at +Z (looking -Z).
            // In Bevy +X is Right, +Z is Back.
            // At yaw=0: x=0, z=d.
            // Bevy coords: Z is South. X is East.
            // If yaw=0 (South), W should move North (-Z).
            // pan.z is -1.

            // Let's use simple trigonometry instead of Quat to be sure
            // Camera forward direction projected on XZ (ignoring pitch)
            // Camera position: (sin(yaw), cos(yaw)) * d.
            // Forward vector (from Cam to Focus): (-sin(yaw), -cos(yaw)).
            // Right vector (Forward rotated -90 deg): (cos(yaw), -sin(yaw)). (x=cos, z=-sin)

            // Move vector = Forward * -pan.z + Right * pan.x
            let forward = Vec3::new(-orbit.yaw.sin(), 0.0, -orbit.yaw.cos());
            let right = Vec3::new(orbit.yaw.cos(), 0.0, -orbit.yaw.sin());

            let move_vec = forward * -pan.z + right * pan.x;
            orbit.focus += move_vec;
            needs_update = true;
        }

        // Rotate on mouse drag (left or right button)
        if (mouse_button.pressed(MouseButton::Left) || mouse_button.pressed(MouseButton::Right))
            && mouse_motion.delta != Vec2::ZERO
        {
            // Sensitivity (radians per pixel)
            const SENSITIVITY: f32 = 0.005;

            // Update pitch (up/down) with clamping
            orbit.pitch = (orbit.pitch - mouse_motion.delta.y * SENSITIVITY).clamp(
                -std::f32::consts::FRAC_PI_2 + 0.1,
                std::f32::consts::FRAC_PI_2 - 0.1,
            );

            // Update yaw (left/right) - full rotation
            orbit.yaw -= mouse_motion.delta.x * SENSITIVITY;
            needs_update = true;
        }

        // Update camera position if anything changed
        if needs_update {
            // Calculate new camera position using spherical coordinates around the focus point
            let x = orbit.focus.x + orbit.distance * orbit.pitch.cos() * orbit.yaw.sin();
            let y = orbit.focus.y + orbit.distance * orbit.pitch.sin();
            let z = orbit.focus.z + orbit.distance * orbit.pitch.cos() * orbit.yaw.cos();

            // Update transform to orbit around focus
            transform.translation = Vec3::new(x, y, z);
            transform.look_at(orbit.focus, Vec3::Y);
        }
    }
}

/// Setup the 3D scene with only the selected piece
fn setup_piece_viewer_scene(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
    selected_piece: Res<SelectedPieceInfo>,
    mut viewer_state: ResMut<PieceViewerState>,
) {
    // Get piece info or use defaults
    let piece_type = selected_piece.piece_type.unwrap_or(PieceType::King);
    let piece_color = selected_piece.piece_color.unwrap_or(PieceColor::White);

    info!(
        "[PIECE_VIEWER] Setting up scene for {:?} {:?}",
        piece_color, piece_type
    );

    // === PIECE MESHES ===
    let piece_meshes = PieceMeshes {
        king: asset_server.load("models/chess_kit/pieces.glb#Mesh0/Primitive0"),
        king_cross: asset_server.load("models/chess_kit/pieces.glb#Mesh1/Primitive0"),
        pawn: asset_server.load("models/chess_kit/pieces.glb#Mesh2/Primitive0"),
        knight_1: asset_server.load("models/chess_kit/pieces.glb#Mesh3/Primitive0"),
        knight_2: asset_server.load("models/chess_kit/pieces.glb#Mesh4/Primitive0"),
        rook: asset_server.load("models/chess_kit/pieces.glb#Mesh5/Primitive0"),
        bishop: asset_server.load("models/chess_kit/pieces.glb#Mesh6/Primitive0"),
        queen: asset_server.load("models/chess_kit/pieces.glb#Mesh7/Primitive0"),
        human: asset_server.load("models/human.obj"),
    };

    // Spawn only the selected piece at pyramid top
    let piece_position = Vec3::new(0.0, 8.5, 0.0); // Top of the pyramid

    let material = materials.add(StandardMaterial {
        base_color: if piece_color == PieceColor::White {
            Color::WHITE
        } else {
            Color::BLACK
        },
        ..default()
    });

    // Add point light highlighting the pyramid top
    commands.spawn((
        PointLight {
            intensity: 8000.0,
            range: 50.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(0.0, 10.0, 5.0),
        DespawnOnExit(MenuState::PieceViewer),
    ));

    // Spawn only the selected piece at pyramid top
    let piece_position = Vec3::new(0.0, 0.25, 0.0); // Top of the pyramid (Layer 0 is at Y=0.25)

    let piece_entity = spawn_viewer_model(
        &mut commands,
        &piece_meshes,
        material,
        piece_color,
        ViewerModelType::ChessPiece,
        piece_type,
        piece_position, // On top of the pyramid
        MenuState::PieceViewer,
    );

    // Store entity and initialize material values
    viewer_state.selected_entity = Some(piece_entity);
    viewer_state.model_type = ViewerModelType::ChessPiece;
    viewer_state.default_base_color = if piece_color == PieceColor::White {
        [1.0, 1.0, 1.0]
    } else {
        [0.0, 0.0, 0.0]
    };
    viewer_state.default_metallic = 0.0;
    viewer_state.default_perceptual_roughness = 0.5;
    viewer_state.default_emissive = [0.0, 0.0, 0.0, 1.0];
    viewer_state.default_reflectance = 0.5;

    viewer_state.base_color = viewer_state.default_base_color;
    viewer_state.metallic = viewer_state.default_metallic;
    viewer_state.perceptual_roughness = viewer_state.default_perceptual_roughness;
    viewer_state.emissive = viewer_state.default_emissive;
    viewer_state.reflectance = viewer_state.default_reflectance;

    // Store meshes resource for model switching
    commands.insert_resource(piece_meshes);

    // Lights are reused from Main Menu scene - no need to spawn new ones
    info!("[PIECE_VIEWER] Scene setup complete (reusing Main Menu lights)");
}

/// Helper struct for piece meshes
#[derive(Resource, Clone)]
struct PieceMeshes {
    king: Handle<Mesh>,
    king_cross: Handle<Mesh>,
    pawn: Handle<Mesh>,
    knight_1: Handle<Mesh>,
    knight_2: Handle<Mesh>,
    rook: Handle<Mesh>,
    bishop: Handle<Mesh>,
    queen: Handle<Mesh>,
    human: Handle<Mesh>,
}

/// Helper to spawn a piece in the viewer
/// Helper to spawn a piece in the viewer
fn spawn_viewer_model(
    commands: &mut Commands,
    meshes: &PieceMeshes,
    material: Handle<StandardMaterial>,
    color: PieceColor,
    model_type: ViewerModelType,
    piece_type: PieceType,
    position: Vec3,
    despawn_state: MenuState,
) -> Entity {
    // Helper function to get rotation for piece based on color
    fn piece_rotation(color: PieceColor) -> Quat {
        match color {
            PieceColor::White => Quat::IDENTITY,
            PieceColor::Black => Quat::from_rotation_y(std::f32::consts::PI), // 180 degrees
        }
    }

    let rotation = piece_rotation(color);

    if model_type == ViewerModelType::Human {
        let scale = Vec3::splat(0.2); // Match scale of chess pieces

        return commands
            .spawn((
                Transform::from_translation(position)
                    .with_rotation(rotation)
                    .with_scale(scale),
                Visibility::Inherited,
                Name::new("Viewer Human"),
                DespawnOnExit(despawn_state),
            ))
            .with_children(|parent| {
                parent.spawn((
                    Mesh3d(meshes.human.clone()),
                    MeshMaterial3d(material),
                    Transform::from_xyz(0.0, 0.0, 0.0), // Adjust vertical position
                ));
            })
            .id();
    }

    // Default: Chess Piece
    let scale = Vec3::splat(0.2);
    let piece_name = format!("Viewer {:?} {:?}", color, piece_type);

    match piece_type {
        PieceType::King => commands
            .spawn((
                Transform::from_translation(position)
                    .with_rotation(rotation)
                    .with_scale(scale),
                Visibility::Inherited,
                Name::new(piece_name.clone()),
                DespawnOnExit(despawn_state),
                Piece {
                    color,
                    piece_type,
                    x: 0,
                    y: 0,
                },
            ))
            .with_children(|parent| {
                parent.spawn((
                    Mesh3d(meshes.king.clone()),
                    MeshMaterial3d(material.clone()),
                    Transform::from_translation(Vec3::new(-0.2, 0.0, -1.9)),
                ));
                parent.spawn((
                    Mesh3d(meshes.king_cross.clone()),
                    MeshMaterial3d(material),
                    Transform::from_translation(Vec3::new(-0.2, 0.0, -1.9)),
                ));
            })
            .id(),
        PieceType::Queen => commands
            .spawn((
                Transform::from_translation(position)
                    .with_rotation(rotation)
                    .with_scale(scale),
                Visibility::Inherited,
                Name::new(piece_name.clone()),
                DespawnOnExit(despawn_state),
                Piece {
                    color,
                    piece_type,
                    x: 0,
                    y: 0,
                },
            ))
            .with_children(|parent| {
                parent.spawn((
                    Mesh3d(meshes.queen.clone()),
                    MeshMaterial3d(material),
                    Transform::from_translation(Vec3::new(-0.2, 0.0, -0.95)),
                ));
            })
            .id(),
        PieceType::Rook => commands
            .spawn((
                Transform::from_translation(position)
                    .with_rotation(rotation)
                    .with_scale(scale),
                Visibility::Inherited,
                Name::new(piece_name.clone()),
                DespawnOnExit(despawn_state),
                Piece {
                    color,
                    piece_type,
                    x: 0,
                    y: 0,
                },
            ))
            .with_children(|parent| {
                parent.spawn((
                    Mesh3d(meshes.rook.clone()),
                    MeshMaterial3d(material),
                    Transform::from_translation(Vec3::new(-0.1, 0.0, 1.8)),
                ));
            })
            .id(),
        PieceType::Bishop => commands
            .spawn((
                Transform::from_translation(position)
                    .with_rotation(rotation)
                    .with_scale(scale),
                Visibility::Inherited,
                Name::new(piece_name.clone()),
                DespawnOnExit(despawn_state),
                Piece {
                    color,
                    piece_type,
                    x: 0,
                    y: 0,
                },
            ))
            .with_children(|parent| {
                parent.spawn((
                    Mesh3d(meshes.bishop.clone()),
                    MeshMaterial3d(material),
                    Transform::from_translation(Vec3::new(-0.1, 0.0, 0.0)),
                ));
            })
            .id(),
        PieceType::Knight => commands
            .spawn((
                Transform::from_translation(position)
                    .with_rotation(rotation)
                    .with_scale(scale),
                Visibility::Inherited,
                Name::new(piece_name.clone()),
                DespawnOnExit(despawn_state),
                Piece {
                    color,
                    piece_type,
                    x: 0,
                    y: 0,
                },
            ))
            .with_children(|parent| {
                parent.spawn((
                    Mesh3d(meshes.knight_1.clone()),
                    MeshMaterial3d(material.clone()),
                    Transform::from_translation(Vec3::new(-0.2, 0.0, 0.9)),
                ));
                parent.spawn((
                    Mesh3d(meshes.knight_2.clone()),
                    MeshMaterial3d(material),
                    Transform::from_translation(Vec3::new(-0.2, 0.0, 0.9)),
                ));
            })
            .id(),
        PieceType::Pawn => commands
            .spawn((
                Transform::from_translation(position)
                    .with_rotation(rotation)
                    .with_scale(scale),
                Visibility::Inherited,
                Name::new(piece_name.clone()),
                DespawnOnExit(despawn_state),
                Piece {
                    color,
                    piece_type,
                    x: 0,
                    y: 0,
                },
            ))
            .with_children(|parent| {
                parent.spawn((
                    Mesh3d(meshes.pawn.clone()),
                    MeshMaterial3d(material),
                    Transform::from_translation(Vec3::new(-0.2, 0.0, 2.6)),
                ));
            })
            .id(),
    }
}

/// Cleanup on exit - remove viewer-specific components and reset camera
/// Cleanup on exit - remove viewer-specific components and reset camera
fn cleanup_piece_viewer(
    persistent_camera: Res<crate::PersistentEguiCamera>,
    mut commands: Commands,
    mut camera_query: Query<
        (&mut Transform, Entity),
        (With<bevy_egui::PrimaryEguiContext>, With<ViewerCamera>),
    >,
    despawn_query: Query<(Entity, &crate::core::DespawnOnExit<crate::core::MenuState>)>,
    children_query: Query<&Children>,
) {
    info!("[PIECE_VIEWER] Cleaning up on exit");

    // Despawn viewer entities
    let mut count = 0;
    for (entity, marker) in despawn_query.iter() {
        if marker.0 == crate::core::MenuState::PieceViewer {
            // Manual recursive despawn
            fn despawn_recursive(
                commands: &mut Commands,
                entity: Entity,
                children_query: &Query<&Children>,
            ) {
                if let Ok(children) = children_query.get(entity) {
                    for child in children.iter() {
                        despawn_recursive(commands, child, children_query);
                    }
                }
                commands.entity(entity).despawn();
            }
            despawn_recursive(&mut commands, entity, &children_query);
            count += 1;
        }
    }
    info!("[PIECE_VIEWER] Despawned {} entities", count);

    // Remove viewer camera components and reset camera transform
    let camera_entity = match persistent_camera.entity {
        Some(entity) => entity,
        None => {
            warn!("[PIECE_VIEWER] WARNING: Persistent camera entity is None during cleanup");
            return;
        }
    };

    match camera_query.get_mut(camera_entity) {
        Ok((mut transform, _)) => {
            // Reset camera to menu position
            *transform = Transform::from_xyz(8.0, 12.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y);

            // Remove viewer-specific components (these may not exist, so we ignore errors)
            commands.entity(camera_entity).remove::<ViewerCamera>();
            commands
                .entity(camera_entity)
                .remove::<PieceViewerOrbitCamera>();
            // commands.entity(camera_entity).remove::<DistanceFog>();

            info!("[PIECE_VIEWER] Camera reset to menu position and viewer components removed");
        }
        Err(e) => {
            warn!("[PIECE_VIEWER] WARNING: Camera entity not found or doesn't have ViewerCamera component: {:?}", e);
            warn!("[PIECE_VIEWER] Attempting to remove components anyway");

            // Try to remove components even if query failed
            commands.entity(camera_entity).remove::<ViewerCamera>();
            commands
                .entity(camera_entity)
                .remove::<PieceViewerOrbitCamera>();
            // commands.entity(camera_entity).remove::<DistanceFog>();
        }
    }

    // Reset SelectedPieceInfo resource
    // (Will be reset by default on next enter, but we can explicitly clear it)
    // Note: Resources persist across state transitions, so we don't need to reset here
    // The resource will be used next time the viewer is opened
}

/// Wrapper for piece_viewer_ui that handles Result
fn piece_viewer_ui_wrapper(
    contexts: EguiContexts,
    next_state: ResMut<NextState<MenuState>>,
    mut viewer_state: ResMut<PieceViewerState>,
    mut selected_piece: ResMut<SelectedPieceInfo>,
) {
    match piece_viewer_ui(contexts, next_state, viewer_state, selected_piece) {
        Ok(()) => {
            // UI rendered successfully
        }
        Err(e) => {
            error!("[PIECE_VIEWER] UI system error: {:?}", e);
            error!("[PIECE_VIEWER] This usually means the Egui context is not available.");
            error!("[PIECE_VIEWER] The piece viewer UI will not be displayed.");
        }
    }
}

/// Main piece viewer UI system
fn piece_viewer_ui(
    mut contexts: EguiContexts,
    mut next_state: ResMut<NextState<MenuState>>,
    mut viewer_state: ResMut<PieceViewerState>,
    mut selected_piece: ResMut<SelectedPieceInfo>,
) -> Result<(), bevy::ecs::query::QuerySingleError> {
    let ctx = contexts.ctx_mut()?;

    // Right-side control panel with transparent background
    egui::SidePanel::right("piece_viewer_controls")
        .resizable(false)
        .default_width(250.0)
        .frame(egui::Frame {
            fill: egui::Color32::TRANSPARENT,
            inner_margin: egui::Margin::same(15),
            ..Default::default()
        })
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                // Title
                ui.heading(TextStyle::heading("Piece Viewer", TextSize::LG));
                Layout::item_space(ui);

                if viewer_state.model_type == ViewerModelType::ChessPiece {
                    let mut current_type = selected_piece.piece_type.unwrap_or(PieceType::King);
                    let mut current_color = selected_piece.piece_color.unwrap_or(PieceColor::White);
                    let mut changed = false;

                    // Color Selection
                    ui.horizontal(|ui| {
                        ui.label(TextStyle::body("Color:"));
                        egui::ComboBox::from_id_source("piece_color")
                            .selected_text(format!("{:?}", current_color))
                            .show_ui(ui, |ui| {
                                if ui
                                    .selectable_value(
                                        &mut current_color,
                                        PieceColor::White,
                                        "White",
                                    )
                                    .clicked()
                                {
                                    changed = true;
                                }
                                if ui
                                    .selectable_value(
                                        &mut current_color,
                                        PieceColor::Black,
                                        "Black",
                                    )
                                    .clicked()
                                {
                                    changed = true;
                                }
                            });
                    });

                    Layout::item_space(ui);

                    // Type Selection
                    ui.horizontal(|ui| {
                        ui.label(TextStyle::body("Type:"));
                        egui::ComboBox::from_id_source("piece_type")
                            .selected_text(format!("{:?}", current_type))
                            .show_ui(ui, |ui| {
                                let types = [
                                    PieceType::King,
                                    PieceType::Queen,
                                    PieceType::Rook,
                                    PieceType::Bishop,
                                    PieceType::Knight,
                                    PieceType::Pawn,
                                ];
                                for p_type in types {
                                    if ui
                                        .selectable_value(
                                            &mut current_type,
                                            p_type,
                                            format!("{:?}", p_type),
                                        )
                                        .clicked()
                                    {
                                        changed = true;
                                    }
                                }
                            });
                    });

                    if changed {
                        selected_piece.piece_type = Some(current_type);
                        selected_piece.piece_color = Some(current_color);
                    }
                } else {
                    ui.label(TextStyle::body("Viewing: Human Model"));
                }

                Layout::item_space(ui);

                // Model Selection
                ui.heading(TextStyle::heading("Models", TextSize::MD));
                Layout::small_space(ui);

                ui.horizontal(|ui| {
                    if ui
                        .selectable_label(
                            viewer_state.model_type == ViewerModelType::ChessPiece,
                            "Chess Piece",
                        )
                        .clicked()
                    {
                        viewer_state.model_type = ViewerModelType::ChessPiece;
                    }
                    if ui
                        .selectable_label(
                            viewer_state.model_type == ViewerModelType::Human,
                            "Human",
                        )
                        .clicked()
                    {
                        viewer_state.model_type = ViewerModelType::Human;
                    }
                });

                Layout::item_space(ui);

                // Rotation instructions
                ui.heading(TextStyle::heading("Controls", TextSize::MD));
                Layout::small_space(ui);
                ui.label(TextStyle::caption("🖱️ Drag to rotate"));
                ui.label(TextStyle::caption("🖱️ Scroll to zoom"));
                ui.label(TextStyle::caption("⌨️ WASD to pan"));
                Layout::item_space(ui);

                Layout::section_space(ui);

                // Back button
                if StyledButton::secondary(ui, "← Back to Menu").clicked() {
                    next_state.set(MenuState::Main);
                }

                Layout::section_space(ui);

                // === MATERIAL PROPERTIES ===
                ui.heading(TextStyle::heading("Material Properties", TextSize::MD));
                Layout::item_space(ui);

                // Base Color
                ui.label(TextStyle::body("Base Color"));
                ui.horizontal(|ui| {
                    ui.label(TextStyle::caption("R:"));
                    ui.add_sized(
                        [80.0, 0.0],
                        egui::Slider::new(&mut viewer_state.base_color[0], 0.0..=1.0)
                            .text("")
                            .show_value(false),
                    );
                    ui.label(TextStyle::caption(format!(
                        "{:.2}",
                        viewer_state.base_color[0]
                    )));
                });
                ui.horizontal(|ui| {
                    ui.label(TextStyle::caption("G:"));
                    ui.add_sized(
                        [80.0, 0.0],
                        egui::Slider::new(&mut viewer_state.base_color[1], 0.0..=1.0)
                            .text("")
                            .show_value(false),
                    );
                    ui.label(TextStyle::caption(format!(
                        "{:.2}",
                        viewer_state.base_color[1]
                    )));
                });
                ui.horizontal(|ui| {
                    ui.label(TextStyle::caption("B:"));
                    ui.add_sized(
                        [80.0, 0.0],
                        egui::Slider::new(&mut viewer_state.base_color[2], 0.0..=1.0)
                            .text("")
                            .show_value(false),
                    );
                    ui.label(TextStyle::caption(format!(
                        "{:.2}",
                        viewer_state.base_color[2]
                    )));
                });

                // Color picker
                egui::widgets::color_picker::color_edit_button_rgb(
                    ui,
                    &mut viewer_state.base_color,
                );

                Layout::item_space(ui);

                // Metallic
                ui.label(TextStyle::body("Metallic"));
                ui.add(
                    egui::Slider::new(&mut viewer_state.metallic, 0.0..=1.0)
                        .show_value(true)
                        .text(""),
                );

                Layout::item_space(ui);

                // Perceptual Roughness
                ui.label(TextStyle::body("Roughness"));
                ui.add(
                    egui::Slider::new(&mut viewer_state.perceptual_roughness, 0.089..=1.0)
                        .show_value(true)
                        .text(""),
                );

                Layout::item_space(ui);

                // Emissive
                ui.label(TextStyle::body("Emissive Color"));
                ui.horizontal(|ui| {
                    ui.label(TextStyle::caption("R:"));
                    ui.add_sized(
                        [60.0, 0.0],
                        egui::Slider::new(&mut viewer_state.emissive[0], 0.0..=1.0)
                            .text("")
                            .show_value(false),
                    );
                    ui.label(TextStyle::caption(format!(
                        "{:.2}",
                        viewer_state.emissive[0]
                    )));
                });
                ui.horizontal(|ui| {
                    ui.label(TextStyle::caption("G:"));
                    ui.add_sized(
                        [60.0, 0.0],
                        egui::Slider::new(&mut viewer_state.emissive[1], 0.0..=1.0)
                            .text("")
                            .show_value(false),
                    );
                    ui.label(TextStyle::caption(format!(
                        "{:.2}",
                        viewer_state.emissive[1]
                    )));
                });
                ui.horizontal(|ui| {
                    ui.label(TextStyle::caption("B:"));
                    ui.add_sized(
                        [60.0, 0.0],
                        egui::Slider::new(&mut viewer_state.emissive[2], 0.0..=1.0)
                            .text("")
                            .show_value(false),
                    );
                    ui.label(TextStyle::caption(format!(
                        "{:.2}",
                        viewer_state.emissive[2]
                    )));
                });

                Layout::item_space(ui);

                // Reflectance
                ui.label(TextStyle::body("Reflectance"));
                ui.add(
                    egui::Slider::new(&mut viewer_state.reflectance, 0.0..=1.0)
                        .show_value(true)
                        .text(""),
                );

                Layout::section_space(ui);

                // Reset button
                if StyledButton::secondary(ui, "Reset to Default").clicked() {
                    viewer_state.reset_to_defaults();
                }
            });
        });

    Ok(())
}

/// Update piece materials based on UI changes
fn update_piece_materials(
    viewer_state: ResMut<PieceViewerState>,
    mut material_assets: ResMut<Assets<StandardMaterial>>,
    children_query: Query<&Children>,
    material_query: Query<&MeshMaterial3d<StandardMaterial>>,
) {
    // Update materials if state changed
    if viewer_state.is_changed() {
        let selected_entity = match viewer_state.selected_entity {
            Some(entity) => entity,
            None => {
                // No piece selected yet, this is normal during initialization
                return;
            }
        };

        // Get children component from the selected piece entity
        if let Ok(children_component) = children_query.get(selected_entity) {
            // Iterate over children - Children derefs to &[Entity]
            // children_component.iter() yields &Entity, but Query::get might accept &Entity directly
            for child_entity in children_component.iter() {
                // Try passing &Entity directly - Query::get should accept it
                if let Ok(material_handle) = material_query.get(child_entity) {
                    // Update material asset
                    if let Some(material) = material_assets.get_mut(&material_handle.0) {
                        material.base_color = Color::srgb(
                            viewer_state.base_color[0],
                            viewer_state.base_color[1],
                            viewer_state.base_color[2],
                        );
                        material.metallic = viewer_state.metallic;
                        material.perceptual_roughness = viewer_state.perceptual_roughness;
                        material.emissive = bevy::color::LinearRgba::new(
                            viewer_state.emissive[0],
                            viewer_state.emissive[1],
                            viewer_state.emissive[2],
                            viewer_state.emissive[3],
                        );
                        material.reflectance = viewer_state.reflectance;
                    } else {
                        warn!(
                            "[PIECE_VIEWER] WARNING: Material asset not found for handle {:?}",
                            material_handle.0
                        );
                    }
                }
                // If child doesn't have a material component, skip it silently
                // This is normal for pieces with multiple child meshes where some may not have materials
            }
        } else {
            warn!(
                "[PIECE_VIEWER] WARNING: Failed to get children for piece entity {:?}",
                selected_entity
            );
        }
    }
}

/// System to handle model type switching logic
fn update_viewer_model(
    mut commands: Commands,
    mut viewer_state: ResMut<PieceViewerState>,
    piece_meshes: Res<PieceMeshes>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    selected_piece: Res<SelectedPieceInfo>,
    mut last_model_type: Local<Option<ViewerModelType>>,
) {
    // Initialize local if needed
    if last_model_type.is_none() {
        *last_model_type = Some(viewer_state.model_type);
        return;
    }

    if viewer_state.model_type != last_model_type.unwrap() || selected_piece.is_changed() {
        // Model type changed!
        let new_model = viewer_state.model_type;
        *last_model_type = Some(new_model);

        // Despawn old entity
        if let Some(entity) = viewer_state.selected_entity {
            commands.entity(entity).despawn();
        }

        // Spawn new entity
        let piece_type = selected_piece.piece_type.unwrap_or(PieceType::King);
        let piece_color = selected_piece.piece_color.unwrap_or(PieceColor::White);
        let piece_position = Vec3::new(0.0, 0.25, 0.0);

        let material = materials.add(StandardMaterial {
            base_color: Color::srgb(
                viewer_state.base_color[0],
                viewer_state.base_color[1],
                viewer_state.base_color[2],
            ),
            metallic: viewer_state.metallic,
            perceptual_roughness: viewer_state.perceptual_roughness,
            emissive: bevy::color::LinearRgba::new(
                viewer_state.emissive[0],
                viewer_state.emissive[1],
                viewer_state.emissive[2],
                viewer_state.emissive[3],
            ),
            reflectance: viewer_state.reflectance,
            ..default()
        });

        let piece_entity = spawn_viewer_model(
            &mut commands,
            &piece_meshes,
            material,
            piece_color,
            new_model,
            piece_type,
            piece_position,
            MenuState::PieceViewer,
        );

        viewer_state.selected_entity = Some(piece_entity);
        info!("[PIECE_VIEWER] Switched model to {:?}", new_model);
    }
}
