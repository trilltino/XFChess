
use bevy::prelude::*;
use bevy_mod_picking::prelude::*;
use crate::assets::*;


#[derive(Default)]
#[derive(Resource)]
struct SelectedSquare {
    entity: Option<Entity>,
}

#[derive(Default)]
#[derive(Resource)]
struct SelectedPiece {
    entity: Option<Entity>,
}


#[derive(Bundle)]
struct CustomBundle{
    pbr: PbrBundle,
    pickable: PickableBundle,
    square: Square,
}

#[derive(Component)]
pub struct Square {
    pub x: u8,
    pub y: u8,
}

impl Square {
    pub fn is_yellow(&self) -> bool {
        (self.x + self.y + 1) % 2 == 0
    }
}

pub fn make_pickable(
    mut commands: Commands,
    meshes: Query<Entity, ComplexQuery>,
) {
    for entity in meshes.iter() {
        commands
            .entity(entity)
            .insert((PickableBundle::default(), HIGHLIGHT_TINT.clone()));
    }
}

type ComplexQuery = (With<Handle<Mesh>>, With<Square>, Without<Pickable>);


const HIGHLIGHT_TINT: Highlight<StandardMaterial> = Highlight {
    hovered: Some(HighlightKind::new_dynamic(|matl| StandardMaterial {
        base_color: matl
            .base_color
            .mix(&Color::srgba(-0.5, -0.3, 0.9, 0.8), 0.5), // hovered is blue
        ..matl.to_owned()
    })),
    pressed: Some(HighlightKind::new_dynamic(|matl| StandardMaterial {
        base_color: matl
            .base_color
            .mix(&Color::srgba(-0.4, -0.4, 0.8, 0.8), 0.5), // pressed is a different blue
        ..matl.to_owned()
    })),
    selected: Some(HighlightKind::new_dynamic(|matl| StandardMaterial {
        base_color: matl
            .base_color
            .mix(&Color::srgba(-0.4, 0.8, -0.4, 0.0), 0.5), // selected is green
        ..matl.to_owned()
    })),
};

pub fn create_board(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>, 
    mut materials: ResMut<Assets<StandardMaterial>>,
    selected_square: Res<SelectedSquare>,
    meshes: Query<Entity, (With<Handle<Square>>, Without<Pickable>)>,
    
) {
    let mesh= meshes.add(Plane3d::default().mesh().size(1.0, 1.0));
    let yellow_material = materials.add(Color::srgb(1.0, 1.0, 0.0));
    let black_material = materials.add(Color::BLACK);
    for i in 0..8 {
        for j in 0..8 {
            commands.spawn(CustomBundle {
                pbr: PbrBundle gvbcgcvb{
                    mesh: mesh.clone(),
                    material: if (i + j + 1) % 2 == 0 {
                        yellow_material.clone()
                    } else {
                        black_material.clone()
                    },
                    transform: Transform::from_translation(Vec3::new(i as f32, 0.0, j as f32)),
                    ..Default::default()
                },
                pickable: PickableBundle::default(),
                square: Square { x: i, y: j },
            });
        }
    }
}

fn color_squares(
    mut commands: Commands,
    selected_square: Res<SelectedSquare>,
    meshes: Query<Entity, (With<Handle<Square>>, Without<Pickable>)>,
) {
    for entity in meshes.iter() {
        commands
            .entity(entity)
            .insert(PickableBundle::default());
    }
}

    struct SquareMaterials {
        highlight_color: Handle<StandardMaterial>,
        selected_color: Handle<StandardMaterial>,
        black_color: Handle<StandardMaterial>,
        white_color: Handle<StandardMaterial>,
    }
    
    impl FromWorld for SquareMaterials {
        fn from_world(world: &mut World) -> Self {
            let mut materials = world
                .get_resource_mut::<Assets<StandardMaterial>>()
                .expect("Failed to get Assets<StandardMaterial> resource");
            SquareMaterials {
                highlight_color: materials.add(StandardMaterial::from(Color::srgb(0.8, 0.3, 0.3))),
                selected_color: materials.add(StandardMaterial::from(Color::srgb(0.9, 0.1, 0.1))),
                black_color: materials.add(Color::srgb(0., 0.1, 0.1)),
                white_color: materials.add(Color::srgb(1., 0.9, 0.9)),
            }
        }
    }


pub fn select_square(
    mut commands: Commands,
    meshes: Query<Entity, (With<Handle<Mesh>>, Without<Pickable>)>,
) {

    for entity in meshes.iter() {
        commands
            .entity(entity)
            .insert(PickableBundle::default());
    }
}

    pub struct BoardPlugin;
    impl Plugin for BoardPlugin {
        fn build(&self, app: &mut App) {
            app.add_systems(Startup, |commands: Commands, meshes: ResMut<Assets<Mesh>>, materials: ResMut<Assets<StandardMaterial>>| {
                create_board(commands, meshes, materials);
            })
            .add_systems(Update, select_square)
            .add_systems(Update, color_squares)
            .add_systems(Update, make_pickable);
        }
    }


