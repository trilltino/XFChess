use crate::game::components::{Captured, FadingCapture, PieceMoveAnimation};
use crate::game::resources::{CurrentTurn, GameTimer, PendingTurnAdvance, Selection};
use crate::rendering::pieces::Piece;
use crate::rendering::utils::{ReturnMaterials, Square, SquareMaterials};
use bevy::prelude::*;

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
    mut squares_query: Query<(Entity, &Square, &mut MeshMaterial3d<StandardMaterial>)>,
) {
    for (_, square, mut material) in squares_query.iter_mut() {
        let pos = (square.x, square.y);

        // Check if this square should be highlighted
        let should_highlight = selection.is_selected()
            && (
                selection.selected_position == Some(pos) || // Selected square
            selection.possible_moves.contains(&pos)
                // Valid move destination
            );

        if should_highlight {
            // Handle is Clone (not Copy), need .clone() from Res
            material.0 = square_materials.hover_matl.clone();
        } else {
            // Restore original color
            material.0 = return_materials.get_original_material(square, &square_materials);
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
            // Rate-limited warning to avoid spam
            let target = Vec3::new(piece.x as f32, 0.0, piece.y as f32);
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

/// System to animate captured pieces fading out
///
/// Pieces with FadingCapture component fade out over time, then move to capture zone.
pub fn animate_capture_fade(
    time: Res<Time>,
    mut commands: Commands,
    mut query: Query<(
        Entity,
        &mut FadingCapture,
        &MeshMaterial3d<StandardMaterial>,
    )>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (entity, mut fading, material_handle) in query.iter_mut() {
        fading.timer.tick(time.delta());

        // Calculate fade progress (1.0 = start, 0.0 = end)
        let alpha = 1.0 - fading.timer.fraction();

        // Update material alpha
        if let Some(material) = materials.get_mut(&material_handle.0) {
            material.base_color = material.base_color.with_alpha(alpha);
            material.alpha_mode = bevy::render::alpha::AlphaMode::Blend;
        }

        // When fade completes, move to capture zone
        if fading.timer.finished() {
            commands.entity(entity).remove::<FadingCapture>();
            commands.entity(entity).insert((
                Transform::from_translation(fading.capture_zone_pos),
                Captured,
            ));

            // Reset alpha to 1.0 for display in capture zone
            if let Some(material) = materials.get_mut(&material_handle.0) {
                material.base_color = material.base_color.with_alpha(1.0);
            }
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
        Name::new("Global Background"),
    ));

    // Global ambient light - set to pure black (changed from blue-gray)
    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.0, 0.0, 0.0), // Pure black - no ambient light tint
        brightness: 0.0,                   // Zero brightness
        affects_lightmapped_meshes: true,
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
                illuminance: 8000.0,
                shadows_enabled: true,
                color: Color::srgb(1.0, 0.98, 0.95), // Warm white
                ..default()
            },
            Transform::from_rotation(Quat::from_euler(
                EulerRot::XYZ,
                -std::f32::consts::FRAC_PI_4,
                std::f32::consts::FRAC_PI_4,
                0.0,
            )),
            DespawnOnExit(GameState::InGame),
            Name::new("Main Directional Light"),
        ));

        // Fill light (reduces harsh shadows)
        commands.spawn((
            PointLight {
                intensity: 500_000.0,
                color: Color::srgb(0.9, 0.9, 1.0), // Slightly blue
                shadows_enabled: false,
                range: 30.0,
                ..default()
            },
            Transform::from_xyz(-10.0, 10.0, 10.0),
            DespawnOnExit(GameState::InGame),
            Name::new("Fill Light"),
        ));
    }

    // Note: Ambient light is set globally in setup_global_scene (Startup)
}
