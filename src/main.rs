    use bevy::prelude::*;
    use bevy_inspector_egui::quick::WorldInspectorPlugin;

    mod board;
    use board::*;
    
    
    mod pieces;
    use pieces::*;

    mod pointer_events;
    use pointer_events::*;

    
    mod egui_ui;
    use egui_ui::*;

    //mod debug_camera;
    //use debug_camera::*;



    const WINDOW_WIDTH : f32 = 1366.0;
    const WINDOW_HEIGHT: f32 = 768.0;
     fn main()  
    { 
        let window = Window { resolution: ( WINDOW_WIDTH, WINDOW_HEIGHT ).into(), ..default() };
        let primary_window = Some ( window );
        App::new()
            .add_plugins( DefaultPlugins.set( WindowPlugin { primary_window, ..default()}))
            .add_plugins(MeshPickingPlugin)
            //.add_plugins(CameraPlugin)
            .add_plugins(WorldInspectorPlugin::new())
            .add_plugins(BoardPlugin)
            .add_systems(Update, draw_mesh_intersections)
            .add_systems(Startup, setup)
            .add_systems(Startup, |mut commands: Commands, server: Res<AssetServer>, materials: ResMut<Assets<StandardMaterial>>| {
                create_pieces(&mut commands, server, materials);
            })
            .run();
    }

    fn setup(mut commands: Commands,
        mut meshes: ResMut<Assets<Mesh>>,
        mut materials: ResMut<Assets<StandardMaterial>>,
    ) {
        commands.spawn((
            PointLight{
                shadows_enabled: true,
                intensity: 100000.0,
                ..default()
            },
            Transform::from_xyz(4.0, 8.0, 4.0),
        ));
        commands.spawn((
            Camera3d::default(),
            Transform::from_matrix(Mat4::from_rotation_translation(
            Quat::from_xyzw(-0.3, -0.5, -0.3, 0.5).normalize(),
            Vec3::new(-7.0, 20.0, 4.0),
            )),
        ));

        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(2.0, 1.0, 1.0))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Srgba::hex("000000").unwrap().into(),
                unlit: true,
                cull_mode: None,
                ..default()
            })),
            Transform::from_scale(Vec3::splat(1_000_000.0)),
        ));
    }


//5 . Play with audio and lighting. https://bevyengine.org/examples-webgpu/3d-rendering/fog
//6. Define App states