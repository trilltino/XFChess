
use bevy::prelude::*;
use bevy_mod_picking::prelude::*;



mod board;
use board::*;
mod assets;
use assets::*;

const WINDOW_WIDTH : f32 = 1366.0;
const WINDOW_HEIGHT: f32 = 768.0;

fn   main()
{ 
    let window = Window { resolution: ( WINDOW_WIDTH, WINDOW_HEIGHT ).into(), ..default() };
    let primary_window = Some ( window );
    App::new()
        .add_plugins( DefaultPlugins.set( WindowPlugin { primary_window, ..default() } ) )
        .add_plugins(DefaultPickingPlugins)
        .add_plugins(BoardPlugin)
        .add_plugins(PiecesPlugin)
        .insert_resource(DebugPickingMode::Normal)
        .add_systems(Update, select_square)
        .add_systems(Startup, setup)
        .add_systems(Update, make_pickable)
        .add_systems(Startup, |commands: Commands, meshes: ResMut<Assets<Mesh>>, materials: ResMut<Assets<StandardMaterial>>| {
            create_board(commands, meshes, materials);
        })
        .add_systems(Startup, |mut commands: Commands, server: Res<AssetServer>, materials: ResMut<Assets<StandardMaterial>>| {
            create_pieces(&mut commands, server, materials);
        })
        .run();
}

fn setup(
    mut commands: Commands,
) {
   
   
    commands.spawn((
        PointLight{
            shadows_enabled: true,
            ..default
        },
        transform::from_xyz(4.0, 8.0, 4.0),
    )); 

commands.spawn((
    Camera3d::default(),
    Transform::from_xyz(-0.3, -0.5, -0.3).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    }
