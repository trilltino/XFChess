use crate::game::components::{FadingCapture, PieceMoveAnimation};
use crate::game::resources::{CurrentTurn, GameTimer, PendingTurnAdvance, Selection};
use crate::rendering::pieces::{Piece, PIECE_ON_BOARD_Y};
use crate::rendering::utils::{Square, SquareMaterials};
use bevy::prelude::*;

/// Advance the turn immediately in the Execution set (before AI systems run)
/// so the AI sees the new turn in the same frame the player moved.
pub fn flush_pending_turn(
    mut pending_turn: ResMut<PendingTurnAdvance>,
    mut current_turn: ResMut<CurrentTurn>,
    mut game_timer: ResMut<GameTimer>,
) {
    if let Some(pending) = pending_turn.take() {
        game_timer.apply_increment(pending.mover);
        current_turn.switch();
    }
}

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
    squares_query: Query<(&Square, &Children)>,
    mut commands: Commands,
    marker_query: Query<Entity, Or<(With<SelectedBorder>, With<MoveHint>)>>,
) {
    // Despawn old marker entities (SelectedBorder + MoveHint overlays).
    for entity in marker_query.iter() {
        commands.entity(entity).despawn();
    }

    // Spawn new markers based on current selection.
    for (square, _children) in squares_query.iter() {
        let pos = (square.x, square.y);
        let is_selected = selection.selected_position == Some(pos);
        let is_valid_move = selection.is_selected() && selection.possible_moves.contains(&pos);

        if is_selected {
            commands.spawn((
                Mesh3d(square_materials.highlight_mesh.clone()),
                MeshMaterial3d(square_materials.selected_border_matl.clone()),
                Transform::from_translation(Vec3::new(square.x as f32, 0.03, square.y as f32)),
                SelectedBorder,
                Name::new("Selected Border"),
                crate::core::DespawnOnExit(crate::core::GameState::InGame),
            ));
        }

        if is_valid_move {
            commands.spawn((
                Mesh3d(square_materials.hint_mesh.clone()),
                MeshMaterial3d(square_materials.hover_matl.clone()),
                Transform::from_translation(Vec3::new(square.x as f32, 0.04, square.y as f32))
                    .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
                MoveHint,
                Name::new("Move Hint"),
                crate::core::DespawnOnExit(crate::core::GameState::InGame),
            ));
        }
    }
}

/// System to animate piece movement with a smooth arc.
///
/// Each frame, increments `PieceMoveAnimation::elapsed` and interpolates
/// the piece's world position between `start` and `end`:
/// - X/Z slide uses smooth-step easing (slow→fast→slow).
/// - Y uses a parabolic arc peaking at the midpoint for a natural lift.
///
/// The component is removed once `elapsed >= duration`, at which point the
/// piece snaps exactly to `end`.  Pieces without an active animation are
/// kept in sync with their `Piece` logical position each frame.
pub fn animate_piece_movement(
    time: Res<Time>,
    mut commands: Commands,
    mut query: Query<(
        Entity,
        &mut Transform,
        &Piece,
        Option<&mut PieceMoveAnimation>,
    )>,
) {
    let dt = time.delta_secs();
    for (entity, mut transform, piece, animation) in query.iter_mut() {
        if let Some(mut anim) = animation {
            anim.elapsed += dt;

            if anim.elapsed >= anim.duration {
                // Animation complete — snap to exact destination.
                transform.translation = anim.end;
                commands.entity(entity).remove::<PieceMoveAnimation>();
            } else {
                // Smooth-step t for horizontal slide (ease in-out).
                let t_smooth = anim.progress();
                // Linear t for the arc so the peak is always at the midpoint.
                let t_linear = (anim.elapsed / anim.duration).clamp(0.0, 1.0);

                let base = anim.start.lerp(anim.end, t_smooth);
                // Arc height scales with board distance so short moves look natural.
                let dist = (anim.end - anim.start).length();
                let arc_height = (dist * 0.18).clamp(0.15, 0.55);
                let arc_y = arc_height * 4.0 * t_linear * (1.0 - t_linear);

                transform.translation = Vec3::new(base.x, base.y + arc_y, base.z);
            }
        } else {
            let target = Vec3::new(7.0 - piece.x as f32, PIECE_ON_BOARD_Y, piece.y as f32);
            if (transform.translation - target).length() > 0.01 {
                transform.translation = target;
            }
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

    // Match the menu board's ambient (GlobalAmbientLight brightness 95) so the
    // in-game board isn't washed out / over-bright.
    commands.spawn(AmbientLight {
        color: Color::srgb(0.9, 0.92, 1.0),
        brightness: 95.0,
        ..default()
    });
}

/// Marker for the board's camera-following fill light (the "headlamp") that
/// keeps pieces evenly lit from the viewer's side as the camera orbits.
#[derive(Component)]
pub(crate) struct CameraFollowLight;

/// Setup game scene when entering InGame state
///
/// Spawns the game camera, lighting, and chess board.
pub fn setup_game_scene(
    mut commands: Commands,
    view_mode: Res<crate::game::view_mode::ViewMode>,
    mut global_ambient: ResMut<bevy::light::GlobalAmbientLight>,
) {
    use crate::core::DespawnOnExit;
    use crate::core::GameState;

    // Reset global ambient so menu lighting never bleeds into the game.
    global_ambient.color = Color::WHITE;
    global_ambient.brightness = 0.0;

    // Set background color based on view mode
    if view_mode.is_templeos() {
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
    if !view_mode.is_templeos() {
        // Key light is the overhead "Angel Light" (2M, shadows on, spawned in
        // `game_init`) — camera-independent. This is the *fill*: a camera-following
        // "headlamp" so the viewer-facing side of every piece stays evenly lit no
        // matter how the player orbits/zooms. Its position is updated each frame by
        // `update_board_fill_light`. No fixed directional/fill (those over-brightened
        // the board and lit unevenly as the camera moved).
        commands.spawn((
            PointLight {
                intensity: 600_000.0,
                range: 80.0,
                color: Color::srgb(0.95, 0.96, 1.0),
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_xyz(3.5, 12.0, 3.5),
            CameraFollowLight,
            DespawnOnExit(GameState::InGame),
            Name::new("Board Fill Light (camera-follow)"),
        ));
    }

    // Note: Ambient light is set globally in setup_global_scene (Startup)
}

/// Keeps the board fill light at the viewer's position so pieces are lit evenly
/// from the camera's side no matter how the player orbits or zooms. The overhead
/// "Angel Light" and the ambient stay camera-independent; this is the moving fill.
pub fn update_board_fill_light(
    persistent_camera: Res<crate::PersistentEguiCamera>,
    cam_q: Query<&Transform, Without<CameraFollowLight>>,
    mut light_q: Query<&mut Transform, With<CameraFollowLight>>,
) {
    let Some(cam_entity) = persistent_camera.entity else {
        return;
    };
    let Ok(cam) = cam_q.get(cam_entity) else {
        return;
    };
    // Sit just above the camera so the viewer-facing side of every piece is lit.
    let pos = cam.translation + Vec3::Y * 2.0;
    for mut t in &mut light_q {
        t.translation = pos;
    }
}
