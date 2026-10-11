use super::*;

#[derive(Component)]
pub struct MenuBg;

#[derive(Resource, Default)]
pub struct MenuBgPiecesSpawned(pub bool);

pub const BOARD_CAM: Vec3 = Vec3::new(3.5, 14.0, -16.0);

pub const BOARD_CENTER: Vec3 = Vec3::new(3.5, 0.0, 3.5);

#[derive(Resource)]
pub struct MenuCameraOrbit {
    pub angle: f32,
    pub radius: f32,
    pub height: f32,
    pub speed: f32,
    pub ortho: bool,
}

impl Default for MenuCameraOrbit {
    fn default() -> Self {
        Self {
            angle: 0.0,
            radius: 16.0,
            height: 14.0,
            speed: 0.10,
            ortho: false,
        }
    }
}

pub fn spawn_menu_bg_board(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mesh = meshes.add(Cuboid::new(1.0, 0.1, 1.0));

    // Use the same lit materials as gameplay to preserve board shading and piece shadows.
    let light = materials.add(StandardMaterial {
        base_color: Color::srgb(0.97, 0.97, 0.88), // Cream
        ..default()
    });
    let dark = materials.add(StandardMaterial {
        base_color: Color::srgb(0.52, 0.65, 0.40), // Green
        ..default()
    });

    for rank in 0..8u8 {
        for file in 0..8u8 {
            let mat = if (file + rank) % 2 == 0 {
                light.clone()
            } else {
                dark.clone()
            };
            commands.spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(mat),
                Transform::from_xyz(7.0 - file as f32, 0.0, rank as f32),
                MenuBg,
                DespawnOnExit(GameState::MainMenu),
                Name::new(format!("MenuBg-{}{}", (b'a' + file) as char, rank + 1)),
            ));
        }
    }
}

pub fn spawn_menu_bg_pieces(
    mut commands: Commands,
    piece_meshes: Option<Res<PieceMeshes>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut spawned: ResMut<MenuBgPiecesSpawned>,
    mut anim: ResMut<super::super::board_animation::BoardAnimator>,
    existing: Query<(), With<super::super::board_animation::MenuBgPieceHome>>,
) {
    if !existing.is_empty() {
        return; // ambient pieces already present
    }
    let Some(pm) = piece_meshes else {
        return; // meshes not loaded yet — retry next frame
    };

    // Give pieces separate materials so capture fading changes only the captured piece.
    let white_mat = || crate::rendering::pieces::white_piece_material();
    let black_mat = || crate::rendering::pieces::black_piece_material();

    const BACK: [PieceType; 8] = [
        PieceType::Rook,
        PieceType::Knight,
        PieceType::Bishop,
        PieceType::Queen,
        PieceType::King,
        PieceType::Bishop,
        PieceType::Knight,
        PieceType::Rook,
    ];
    let rot_w = Quat::IDENTITY;
    let rot_b = Quat::from_rotation_y(std::f32::consts::PI);
    // Knights face +X in the GLB, so need a 90° offset to face the opponent
    let rot_w_knight = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    let rot_b_knight = Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2);

    for (f, &pt) in BACK.iter().enumerate() {
        let file = f as u8;
        let (wr, br) = if pt == PieceType::Knight {
            (rot_w_knight, rot_b_knight)
        } else {
            (rot_w, rot_b)
        };
        // White back rank 0
        let ew = commands
            .spawn((
                Mesh3d(pm.get(pt, PieceColor::White)),
                MeshMaterial3d(materials.add(white_mat())),
                Transform::from_xyz(7.0 - f as f32, 0.05, 0.0).with_rotation(wr),
                Visibility::Visible,
                MenuBg,
                DespawnOnExit(GameState::MainMenu),
                super::super::board_animation::MenuBgPieceHome { file, rank: 0 },
            ))
            .id();
        anim.board[0][f] = Some(ew);

        // Black back rank 7
        let eb = commands
            .spawn((
                Mesh3d(pm.get(pt, PieceColor::Black)),
                MeshMaterial3d(materials.add(black_mat())),
                Transform::from_xyz(7.0 - f as f32, 0.05, 7.0).with_rotation(br),
                Visibility::Visible,
                MenuBg,
                DespawnOnExit(GameState::MainMenu),
                super::super::board_animation::MenuBgPieceHome { file, rank: 7 },
            ))
            .id();
        anim.board[7][f] = Some(eb);
    }

    for f in 0..8usize {
        let file = f as u8;
        // White pawns rank 1
        let ewp = commands
            .spawn((
                Mesh3d(pm.get(PieceType::Pawn, PieceColor::White)),
                MeshMaterial3d(materials.add(white_mat())),
                Transform::from_xyz(7.0 - f as f32, 0.05, 1.0).with_rotation(rot_w),
                Visibility::Visible,
                MenuBg,
                DespawnOnExit(GameState::MainMenu),
                super::super::board_animation::MenuBgPieceHome { file, rank: 1 },
            ))
            .id();
        anim.board[1][f] = Some(ewp);

        // Black pawns rank 6
        let ebp = commands
            .spawn((
                Mesh3d(pm.get(PieceType::Pawn, PieceColor::Black)),
                MeshMaterial3d(materials.add(black_mat())),
                Transform::from_xyz(7.0 - f as f32, 0.05, 6.0).with_rotation(rot_b),
                Visibility::Visible,
                MenuBg,
                DespawnOnExit(GameState::MainMenu),
                super::super::board_animation::MenuBgPieceHome { file, rank: 6 },
            ))
            .id();
        anim.board[6][f] = Some(ebp);
    }

    spawned.0 = true;
    // Fresh starting position ⇒ reset the replay so a (re)spawn always starts the
    // Immortal-Zugzwang sequence from move 1 rather than resuming a stale ply.
    anim.ply_index = 0;
    anim.reset = super::super::board_animation::ResetPhase::Idle;
    anim.move_timer = 2.5;
    anim.active = true;
}

pub fn purge_stale_lights(
    mut commands: Commands,
    mut global_ambient: ResMut<bevy::light::GlobalAmbientLight>,
    directional: Query<Entity, (With<DirectionalLight>, Without<MenuBg>)>,
    point: Query<Entity, (With<PointLight>, Without<MenuBg>)>,
) {
    for entity in directional.iter().chain(point.iter()) {
        commands.entity(entity).despawn();
    }
    global_ambient.color = Color::WHITE;
    global_ambient.brightness = 95.0;
}

pub fn spawn_menu_bg_lights(
    mut commands: Commands,
    mut global_ambient: ResMut<bevy::light::GlobalAmbientLight>,
    existing: Query<Entity, (With<MenuBg>, With<PointLight>)>,
) {
    for entity in existing.iter() {
        commands.entity(entity).despawn();
    }

    // Match the in-game ambient tone (visual.rs: bluish, brightness 95).
    global_ambient.color = Color::srgb(0.9, 0.92, 1.0);
    global_ambient.brightness = 95.0;

    // Overhead point light — identical to the in-game "Angel Light" (game_init.rs).
    commands.spawn((
        PointLight {
            intensity: 2_000_000.0,
            range: 100.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(3.5, 20.0, 3.5),
        MenuBg,
        DespawnOnExit(GameState::MainMenu),
        Name::new("MenuBg-OverheadLight"),
    ));

    // CameraFollowLight gives the menu the same viewer-facing fill light as gameplay.
    commands.spawn((
        PointLight {
            intensity: 600_000.0,
            range: 80.0,
            color: Color::srgb(0.95, 0.96, 1.0),
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(3.5, 12.0, 3.5),
        crate::game::systems::visual::CameraFollowLight,
        MenuBg,
        DespawnOnExit(GameState::MainMenu),
        Name::new("MenuBg-FillLight (camera-follow)"),
    ));
}

pub fn setup_menu_fog(_commands: Commands, _cam: Res<crate::PersistentEguiCamera>) {}

pub fn orbit_camera_system(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut orbit: ResMut<MenuCameraOrbit>,
    cam: Res<crate::PersistentEguiCamera>,
    mut query: Query<(&mut Transform, &mut Projection), With<Camera3d>>,
) {
    if keyboard.just_pressed(KeyCode::KeyV) {
        orbit.ortho = !orbit.ortho;
        info!(
            "[MENU] Toggled menu board view to {}",
            if orbit.ortho { "orthographic" } else { "orbit" }
        );
    }

    let Some(entity) = cam.entity else { return };
    let Ok((mut t, mut proj)) = query.get_mut(entity) else {
        return;
    };

    if orbit.ortho {
        // Fixed isometric view — mirrors setup_templeos_camera: equal offsets on
        // X/Y/Z from the board centre with a FixedVertical orthographic projection.
        if !matches!(*proj, Projection::Orthographic(_)) {
            *proj = Projection::from(OrthographicProjection {
                scaling_mode: bevy::camera::ScalingMode::FixedVertical {
                    viewport_height: 16.0,
                },
                ..OrthographicProjection::default_3d()
            });
        }
        let offset = 5.0;
        let pos = Vec3::new(BOARD_CENTER.x + offset, offset, BOARD_CENTER.z + offset);
        *t = Transform::from_translation(pos).looking_at(BOARD_CENTER, Vec3::Y);
        return;
    }

    if !matches!(*proj, Projection::Perspective(_)) {
        *proj = Projection::default();
    }
    orbit.angle += orbit.speed * time.delta_secs();
    let x = BOARD_CENTER.x + orbit.radius * orbit.angle.cos();
    let z = BOARD_CENTER.z + orbit.radius * orbit.angle.sin();
    // Look at the true board centre so the board is horizontally centred on
    // screen, symmetric with the centred title.
    *t = Transform::from_translation(Vec3::new(x, orbit.height, z))
        .looking_at(BOARD_CENTER, Vec3::Y);
}
