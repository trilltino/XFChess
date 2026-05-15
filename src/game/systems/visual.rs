use crate::game::components::{FadingCapture, PieceMoveAnimation};
use crate::game::resources::{CurrentTurn, GameTimer, PendingTurnAdvance, Selection};
use crate::rendering::board::board::BoardSquare3DVisual;
use crate::rendering::pieces::{Piece, PIECE_ON_BOARD_Y};
use crate::rendering::utils::{ReturnMaterials, Square, SquareMaterials};
use bevy::prelude::*;

/// Marker component for selected piece borders
#[derive(Component)]
pub struct SelectedBorder;

/// Marker component for legal move hints (3D)
#[derive(Component)]
pub struct MoveHint;


/// System to visually highlight possible moves and selected square
///
/// Updates square materials to provide visual feedback for:
/// - **Selected piece**: Highlights the source square
/// - **Valid moves**: Highlights all legal destination squares
/// - **Restoration**: Restores original colors for unselected squares
///
/// # Execution Order
///
/// Runs in `GameSystems::Visual` set, after all game logic systems.
/// This ensures highlights reflect the current selection state.
///
/// # Performance
///
/// Iterates over all squares each frame. Consider using change detection
/// or event-based updates if this becomes a bottleneck.
pub fn highlight_possible_moves(
    selection: Res<Selection>,
    square_materials: Res<SquareMaterials>,
    return_materials: Res<ReturnMaterials>,
    squares_query: Query<(&Square, &Children)>,
    mut material_query: Query<&mut MeshMaterial3d<StandardMaterial>, With<BoardSquare3DVisual>>,
    mut commands: Commands,
    marker_query: Query<Entity, Or<(With<SelectedBorder>, With<MoveHint>)>>,
) {
    // Clean up old visual markers
    for entity in marker_query.iter() {
        commands.entity(entity).despawn();
    }

    for (square, children) in squares_query.iter() {
        let pos = (square.x, square.y);

        // Check if this is the selected square
        let is_selected = selection.selected_position == Some(pos);
        
        // Check if this is a valid move destination
        let is_valid_move = selection.is_selected() && selection.possible_moves.contains(&pos);

        if is_selected {
            // Add soft black border for selected piece
            commands.spawn((
                Mesh3d(square_materials.highlight_mesh.clone()),
                MeshMaterial3d(square_materials.selected_border_matl.clone()),
                Transform::from_translation(Vec3::new(square.x as f32, 0.02, square.y as f32)),
                SelectedBorder,
                Name::new("Selected Border"),
                crate::core::DespawnOnExit(crate::core::GameState::InGame),
            ));
        }

        if is_valid_move {
            // Add small grey circle for legal move (matching 2D style)
            commands.spawn((
                Mesh3d(square_materials.hint_mesh.clone()),
                MeshMaterial3d(square_materials.hover_matl.clone()),
                Transform::from_translation(Vec3::new(square.x as f32, 0.02, square.y as f32))
                    .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
                MoveHint,
                Name::new("Move Hint"),
                crate::core::DespawnOnExit(crate::core::GameState::InGame),
            ));
        }


        // Update the 3D visual child's material
        for child in children.iter() {
            if let Ok(mut material) = material_query.get_mut(child) {
                if !is_selected {
                    material.0 = return_materials.get_original_material(square, &square_materials);
                }
            }
        }
    }
}

/// System to animate piece movement
///
/// Smoothly interpolates piece positions from their current transform
/// to their target position based on the Piece component.
///
/// # Execution Order
///
/// Runs in `GameSystems::Visual` set, after game logic updates piece
/// positions but before rendering.
///
/// # Animation Behavior
///
/// - Uses linear interpolation with configurable speed
/// - Snaps to final position when within 0.1 units
/// - Handles both movement and capture animations
pub fn animate_piece_movement(
    time: Res<Time>,
    mut commands: Commands,
    mut query: Query<(
        Entity,
        &mut Transform,
        &Piece,
        Option<&mut PieceMoveAnimation>,
    )>,
    mut pending_turn: ResMut<PendingTurnAdvance>,
    mut current_turn: ResMut<CurrentTurn>,
    mut game_timer: ResMut<GameTimer>,
) {
    let mut animation_active = false;
    let mut completed = Vec::new();

    for (entity, mut transform, piece, animation) in query.iter_mut() {
        if let Some(mut animation) = animation {
            animation.elapsed = (animation.elapsed + time.delta_secs()).min(animation.duration);
            let progress = animation.progress();
            transform.translation = animation.start.lerp(animation.end, progress);

            if progress >= 1.0 {
                transform.translation = animation.end;
                completed.push(entity);
            } else {
                animation_active = true;
            }
        } else {
            // Snap to board surface (y = PIECE_ON_BOARD_Y) when not animating.
            // Pieces must sit on top of the board cuboid (top face at y=0.05),
            // not at y=0 which clips them into the board geometry.
            // Integer coordinates match GLB mesh design and board square positions.
            let target = Vec3::new(piece.x as f32, PIECE_ON_BOARD_Y, piece.y as f32);
            if (transform.translation - target).length() > 0.01 {
                transform.translation = target;
            }
        }
    }

    for entity in completed {
        commands.entity(entity).remove::<PieceMoveAnimation>();
    }

    if !animation_active && pending_turn.is_pending() {
        if let Some(pending) = pending_turn.take() {
            let mover = pending.mover;
            game_timer.apply_increment(mover);
            current_turn.switch();

            // Consolidated log: one line instead of three
            debug!(
                "[MOVE] {:?} → {:?} | Move #{} | Times: W={:.1}s B={:.1}s",
                mover,
                current_turn.color,
                current_turn.move_number,
                game_timer.white_time_left,
                game_timer.black_time_left
            );
        }
    }
}

/// System to animate captured pieces with a parabolic arc, spin, and scale-to-zero.
///
/// # Animation phases (all simultaneous over 0.45 s)
///
/// - **Arc**: piece rises to `arc_height` at t=0.5, then falls back toward the board.
///   Uses a parabolic curve: `y_offset = arc_height * 4t(1-t)`.
/// - **Spin**: piece rotates `spin_radians` around its `spin_axis` using smooth-step t.
/// - **Scale**: piece shrinks to zero using smooth-step easing.
pub fn animate_capture_fade(
    time: Res<Time>,
    mut commands: Commands,
    mut query: Query<(Entity, &mut Transform, &mut FadingCapture)>,
) {
    for (entity, mut transform, mut fading) in query.iter_mut() {
        fading.timer.tick(time.delta());

        // t ∈ [0, 1]
        let t = fading.timer.fraction();

        // 1. Position: Slide horizontally and sink vertically
        //    Horizontal slide: 0.6 units along knockback_dir
        //    Vertical sink: start at board height, end at -0.8 (fully submerged)
        let slide_dist = 0.6 * t;
        let sink_y = PIECE_ON_BOARD_Y - (1.0 * t * t); // Quadratic sink for weight
        
        transform.translation = fading.initial_pos 
            + (fading.knockback_dir * slide_dist) 
            + (Vec3::Y * (sink_y - PIECE_ON_BOARD_Y));

        // 2. Rotation: Tilt back based on impact
        //    Tilt up to 25 degrees (0.43 rad) and then settle
        let tilt_angle = 0.43 * t * (1.0 - t) * 4.0; 
        transform.rotation = Quat::from_axis_angle(fading.tilt_axis, tilt_angle);

        // 3. Scale: Slight shrink to emphasize the 'vanishing'
        let scale = 1.0 - (0.3 * t);
        transform.scale = Vec3::splat(scale);

        if fading.timer.just_finished() {
            commands.entity(entity).despawn();
        }
    }
}

/// Setup global scene elements (persistent background, ambient light)
///
/// These elements persist across all game states and provide
/// a base visual environment.
pub fn setup_global_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Global background (pure black environment)
    let background_color = Color::srgb(0.0, 0.0, 0.0); // Pure black

    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(2.0, 1.0, 1.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: background_color,
            unlit: true,
            cull_mode: None,
            ..default()
        })),
        Transform::from_scale(Vec3::splat(1_000_000.0)),
        bevy::picking::Pickable::IGNORE,
        Name::new("Global Background"),
    ));

    // Global ambient light - set to dim gray to prevent crushing blacks
    commands.spawn(AmbientLight {
        color: Srgba::gray(0.2).into(), // Dim gray ambient
        brightness: 200.0,
        affects_lightmapped_meshes: false,
    });
}

/// Setup game scene when entering InGame state
///
/// Spawns the game camera, lighting, and chess board.
pub fn setup_game_scene(mut commands: Commands, view_mode: Res<crate::game::view_mode::ViewMode>) {
    use crate::core::DespawnOnExit;
    use crate::core::GameState;

    // Set background color based on view mode
    if *view_mode == crate::game::view_mode::ViewMode::TempleOS {
        // Vibrant solid yellow background matching reference image (#FFFF00)
        commands.insert_resource(ClearColor(Color::srgb(1.0, 1.0, 0.0))); // Pure yellow #FFFF00
    } else {
        // Default dark background for standard view
        commands.insert_resource(ClearColor(Color::srgb(0.0, 0.0, 0.0))); // Black
    }

    // Setup camera based on view mode
    // TempleOS camera is set up by the board plugin, so we only create standard camera here
    // UPDATE: We now reuse the PersistentEguiCamera for standard view (in setup_game_camera system)
    // so we ONLY need to handle TempleOS specific setup or lights here.

    // lights...

    // Skip lights for TempleOS mode (unlit rendering)
    if *view_mode != crate::game::view_mode::ViewMode::TempleOS {
        // Main directional light (chess tournament lighting)
        commands.spawn((
            DirectionalLight {
                illuminance: 12000.0, // Brighter
                shadows_enabled: true,
                color: Color::srgb(1.0, 1.0, 0.98), // Cleaner white
                ..default()
            },
            Transform::from_xyz(4.0, 15.0, 4.0).looking_at(Vec3::new(3.5, 0.0, 3.5), Vec3::Y), // Overhead centered
            DespawnOnExit(GameState::InGame),
            Name::new("Main Directional Light"),
        ));

        // Fill light (reduces harsh shadows)
        commands.spawn((
            PointLight {
                intensity: 1_000_000.0,            // Stronger fill
                color: Color::srgb(1.0, 1.0, 1.0), // White fill
                shadows_enabled: false,
                range: 100.0,
                ..default()
            },
            Transform::from_xyz(3.5, 10.0, 3.5), // Center fill
            DespawnOnExit(GameState::InGame),
            Name::new("Fill Light"),
        ));
    }

    // Note: Ambient light is set globally in setup_global_scene (Startup)
}
