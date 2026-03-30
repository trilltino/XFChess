use crate::core::{DespawnOnExit, GameState};
use bevy::prelude::*;

pub fn spawn_debug_markers(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let sphere = meshes.add(Sphere::new(0.3));

    let red = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.0, 0.0),
        emissive: LinearRgba::new(1.0, 0.0, 0.0, 1.0),
        ..default()
    });

    let blue = materials.add(StandardMaterial {
        base_color: Color::srgb(0.0, 0.0, 1.0),
        emissive: LinearRgba::new(0.0, 0.0, 1.0, 1.0),
        ..default()
    });

    let green = materials.add(StandardMaterial {
        base_color: Color::srgb(0.0, 1.0, 0.0),
        emissive: LinearRgba::new(0.0, 1.0, 0.0, 1.0),
        ..default()
    });

    // Corner Markers
    let corners = [
        (0.0, 0.0), // a1
        (7.0, 0.0), // h1 (or a8 depending on axis)
        (0.0, 7.0),
        (7.0, 7.0),
    ];

    for (x, z) in corners {
        commands.spawn((
            Mesh3d(sphere.clone()),
            MeshMaterial3d(red.clone()),
            Transform::from_xyz(x, 0.5, z),
            DespawnOnExit(GameState::InGame),
            Name::new(format!("Debug Marker ({}, {})", x, z)),
        ));
    }

    // Center Marker
    commands.spawn((
        Mesh3d(sphere.clone()),
        MeshMaterial3d(blue.clone()),
        Transform::from_xyz(3.5, 0.5, 3.5),
        DespawnOnExit(GameState::InGame),
        Name::new("Debug Marker Center"),
    ));

    // Origin Marker (0,0,0) - Should be a1 center bottom
    commands.spawn((
        Mesh3d(sphere.clone()),
        MeshMaterial3d(green.clone()),
        Transform::from_xyz(0.0, 0.5, 0.0),
        DespawnOnExit(GameState::InGame),
        Name::new("Debug Marker Origin (0,0)"),
    ));
}
