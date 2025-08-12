use bevy::{
    math::ops,
    pbr::{NotShadowCaster, NotShadowReceiver},
    prelude::*,
};

use crate::egui_ui::playgame_ui;
use crate::state_manager::GameState;

pub fn setup_launch_camera(mut commands: Commands, state: ResMut<NextState<GameState>>) {
    commands.spawn((
        Camera3d::default(),
        DistanceFog {
            color: Color::srgb(0.0, 0.0, 0.0),
            ..Default::default()
        },
    ));
}

pub fn setup_pyramid_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let stone = materials.add(StandardMaterial {
        base_color: Srgba::hex("28221B").unwrap().into(),
        perceptual_roughness: 1.0,
        ..default()
    });
    for (x, z) in &[(-1.5, -1.5), (1.5, -1.5), (1.5, 1.5), (-1.5, 1.5)] {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(1.0, 3.0, 1.0))),
            MeshMaterial3d(stone.clone()),
            Transform::from_xyz(*x, 1.5, *z),
        ));
    }

    // orb
    commands.spawn((
        Mesh3d(meshes.add(Sphere::default())),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Srgba::hex("126212CC").unwrap().into(),
            reflectance: 1.0,
            perceptual_roughness: 0.0,
            metallic: 0.5,
            alpha_mode: AlphaMode::Blend,
            ..default()
        })),
        Transform::from_scale(Vec3::splat(1.75)).with_translation(Vec3::new(0.0, 4.0, 0.0)),
        NotShadowCaster,
        NotShadowReceiver,
    ));

    for i in 0..50 {
        let half_size = i as f32 / 2.0 + 3.0;
        let y = -i as f32 / 2.0;
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(2.0 * half_size, 0.5, 2.0 * half_size))),
            MeshMaterial3d(stone.clone()),
            Transform::from_xyz(0.0, y + 0.25, 0.0),
        ));
    }

    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(2.0, 1.0, 1.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Srgba::hex("888888").unwrap().into(),
            unlit: true,
            cull_mode: None,
            ..default()
        })),
        Transform::from_scale(Vec3::splat(1_000_000.0)),
    ));

    commands.spawn((
        PointLight {
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(0.0, 1.0, 0.0),
    ));
}

fn update_system(
    camera: Single<(&mut DistanceFog, &mut Transform)>,
    mut text: Single<&mut Text>,
    time: Res<Time>,
) {
    let now = time.elapsed_secs();
    let delta = time.delta_secs();

    let (mut fog, mut transform) = camera.into_inner();

    // Orbit camera around pyramid
    let orbit_scale = 8.0 + ops::sin(now / 10.0) * 7.0;
    *transform = Transform::from_xyz(
        ops::cos(now / 5.0) * orbit_scale,
        12.0 - orbit_scale / 2.0,
        ops::sin(now / 5.0) * orbit_scale,
    )
    .looking_at(Vec3::ZERO, Vec3::Y);
}

pub struct Launchmenu<S: States> {
    pub state: S,
}

impl<S: States> Plugin for Launchmenu<S> {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                update_system,
                setup_launch_camera,
                setup_pyramid_scene,
                playgame_ui,
            )
                .run_if(in_computed_state::<LaunchMenu>()),
        );
    }
}
