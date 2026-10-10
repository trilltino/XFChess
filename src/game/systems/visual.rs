use crate::game::components::{FadingCapture, PieceMoveAnimation};
use crate::game::resources::{CurrentTurn, GameTimer, PendingTurnAdvance, Selection};
use crate::rendering::pieces::{Piece, PIECE_ON_BOARD_Y};
use crate::rendering::utils::{Square, SquareMaterials};
use bevy::prelude::*;

pub fn flush_pending_turn(
    mut pending_turn: ResMut<PendingTurnAdvance>,
    mut current_turn: ResMut<CurrentTurn>,
    mut game_timer: ResMut<GameTimer>,
) {
    if let Some(pending) = pending_turn.take() {
        let before = (current_turn.color, current_turn.move_number);
        game_timer.apply_increment(pending.mover);
        current_turn.switch();
        // Use warn for turn-change diagnostics so shipped log filters retain them.
        warn!(
            "[TURN] {:?} move {} -> {:?} move {} (mover was {:?})",
            before.0, before.1, current_turn.color, current_turn.move_number, pending.mover
        );
    }
}

#[derive(Component)]
pub struct SelectedBorder;

#[derive(Component)]
pub struct MoveHint;

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
                bevy::camera::visibility::RenderLayers::layer(
                    crate::game::systems::camera::BOARD_LAYER,
                ),
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
                bevy::camera::visibility::RenderLayers::layer(
                    crate::game::systems::camera::BOARD_LAYER,
                ),
            ));
        }
    }
}

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

pub fn animate_capture_fade(
    time: Res<Time>,
    mut commands: Commands,
    mut query: Query<(Entity, &mut Transform, &mut FadingCapture)>,
) {
    for (entity, mut transform, mut fading) in query.iter_mut() {
        fading.timer.tick(time.delta());

        // t ∈ [0, 1]
        let t = fading.timer.fraction();

        let slide_dist = 0.6 * t;
        let sink_y = PIECE_ON_BOARD_Y - (1.0 * t * t); // Quadratic sink for weight

        transform.translation = fading.initial_pos
            + (fading.knockback_dir * slide_dist)
            + (Vec3::Y * (sink_y - PIECE_ON_BOARD_Y));

        // 2. Rotation: Tilt back based on impact
        //    Tilt up to 25 degrees (0.43 rad) and then settle
        let tilt_angle = 0.43 * t * (1.0 - t) * 4.0;
        transform.rotation = Quat::from_axis_angle(fading.tilt_axis, tilt_angle);

        let scale = 1.0 - (0.3 * t);
        transform.scale = Vec3::splat(scale);

        if fading.timer.just_finished() {
            commands.entity(entity).despawn();
        }
    }
}

pub fn setup_global_scene(mut commands: Commands) {
    // Match the menu board's ambient (GlobalAmbientLight brightness 95) so the
    // in-game board isn't washed out / over-bright.
    commands.spawn(AmbientLight {
        color: Color::srgb(0.9, 0.92, 1.0),
        brightness: 95.0,
        ..default()
    });
}

#[derive(Component)]
pub(crate) struct CameraFollowLight;

pub fn setup_game_scene(
    mut commands: Commands,
    view_mode: Res<crate::game::view_mode::ViewMode>,
    mut global_ambient: ResMut<bevy::light::GlobalAmbientLight>,
) {
    use crate::core::DespawnOnExit;
    use crate::core::GameState;

    // Keep the same restrained ambient baseline as the menu so dark pieces and
    // board details remain readable alongside the overhead and fill lights.
    global_ambient.color = Color::srgb(0.9, 0.92, 1.0);
    global_ambient.brightness = 95.0;

    if view_mode.is_templeos() {
        // Vibrant solid yellow background matching reference image (#FFFF00)
        commands.insert_resource(ClearColor(Color::srgb(1.0, 1.0, 0.0))); // Pure yellow #FFFF00
    } else {
        // Default dark background for standard view
        commands.insert_resource(ClearColor(Color::srgb(0.0, 0.0, 0.0))); // Black
    }

    // Standard mode reuses PersistentEguiCamera; only TempleOS needs setup here.

    // lights...

    // Skip lights for TempleOS mode (unlit rendering)
    if !view_mode.is_templeos() {
        // The camera-following fill light complements the fixed overhead key light
        // and keeps viewer-facing pieces lit while orbiting.
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
            bevy::camera::visibility::RenderLayers::layer(
                crate::game::systems::camera::BOARD_LAYER,
            ),
            Name::new("Board Fill Light (camera-follow)"),
        ));
    }

    // Note: Ambient light is set globally in setup_global_scene (Startup)
}

pub fn update_board_fill_light(
    cam_q: Query<
        &Transform,
        (
            With<crate::game::systems::camera::BoardCamera>,
            Without<CameraFollowLight>,
        ),
    >,
    mut light_q: Query<&mut Transform, With<CameraFollowLight>>,
) {
    let Ok(cam) = cam_q.single() else {
        return;
    };
    // Sit just above the camera so the viewer-facing side of every piece is lit.
    let pos = cam.translation + Vec3::Y * 2.0;
    for mut t in &mut light_q {
        t.translation = pos;
    }
}
