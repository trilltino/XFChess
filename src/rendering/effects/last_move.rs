//! Last move highlighting system.

use crate::core::GameSettings;
use crate::game::resources::MoveHistory;
use crate::rendering::utils::SquareMaterials;
use bevy::prelude::*;

/// Marker component for squares showing last move highlight
#[derive(Component)]
pub struct LastMoveHighlight;

/// Marker component for the 3D directional arrow showing the last move vector.
#[derive(Component)]
pub struct LastMoveArrow3D;

/// Pre-allocated mesh and material for the last-move arrow.
/// Created once at startup; reused every move to avoid per-move GPU allocations.
#[derive(Resource)]
pub struct ArrowAssets {
    pub mesh: Handle<Mesh>,
    pub matl: Handle<StandardMaterial>,
}

/// One-time setup system: allocate the arrow assets.
pub fn init_arrow_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    // Unit-length cuboid — actual length set via Transform.scale.x each move.
    let mesh = meshes.add(Cuboid::new(1.0, 0.015, 0.12));
    let matl = mats.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.85, 0.1, 0.75),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    commands.insert_resource(ArrowAssets { mesh, matl });
}

/// Shows/hides last move highlights. Runs only when `MoveHistory` or settings change.
pub fn update_last_move_highlight_system(
    mut commands: Commands,
    settings: Res<GameSettings>,
    move_history: Res<MoveHistory>,
    highlight_query: Query<Entity, With<LastMoveHighlight>>,
    arrow_query: Query<Entity, With<LastMoveArrow3D>>,
    materials: Res<SquareMaterials>,
    arrow_assets: Option<Res<ArrowAssets>>,
) {
    if !move_history.is_changed() && !settings.is_changed() {
        return;
    }

    for entity in highlight_query.iter() {
        commands.entity(entity).despawn();
    }
    for entity in arrow_query.iter() {
        commands.entity(entity).despawn();
    }

    if !settings.highlight_last_move {
        return;
    }

    let Some(last_move) = move_history.last_move() else { return };
    let Some(assets) = arrow_assets else { return };

    for (x, y) in [last_move.from, last_move.to] {
        commands.spawn((
            Mesh3d(materials.highlight_mesh.clone()),
            MeshMaterial3d(materials.hover_matl.clone()),
            Transform::from_translation(Vec3::new(7.0 - x as f32, 0.02, y as f32)),
            LastMoveHighlight,
            bevy::picking::Pickable::IGNORE,
            Name::new("Last Move Highlight"),
            crate::core::DespawnOnExit(crate::core::GameState::InGame),
        ));
    }

    let src = Vec3::new(7.0 - last_move.from.0 as f32, 0.03, last_move.from.1 as f32);
    let dst = Vec3::new(7.0 - last_move.to.0 as f32, 0.03, last_move.to.1 as f32);
    let dir = dst - src;
    let length = dir.length();
    if length > 0.01 {
        let midpoint = (src + dst) * 0.5;
        let angle = dir.xz().to_angle();
        commands.spawn((
            Mesh3d(assets.mesh.clone()),
            MeshMaterial3d(assets.matl.clone()),
            Transform {
                translation: midpoint,
                rotation: Quat::from_rotation_y(-angle),
                scale: Vec3::new(length * 0.85, 1.0, 1.0),
            },
            LastMoveArrow3D,
            bevy::picking::Pickable::IGNORE,
            Name::new("Last Move Arrow"),
            crate::core::DespawnOnExit(crate::core::GameState::InGame),
        ));
    }
}
