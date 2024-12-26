    use bevy::{color::palettes::tailwind::*, picking::pointer::{PointerInteraction, PointerId}, prelude::*};

    #[derive(Component)]
    struct Board;



    #[derive(Component, Debug)]
     struct Square {
        pub x: u8,
        pub y: u8,
    }

    impl Square {
        fn is_white(&self) -> bool {
            (self.x + self.y + 1) % 2 == 0
        }
    }

    #[derive(Resource)]
   pub struct SquareMaterials {
        highlight_color: Handle<StandardMaterial>,
        selected_color: Handle<StandardMaterial>,
        black_color: Handle<StandardMaterial>,
        white_color: Handle<StandardMaterial>,
    }
    
    
    impl FromWorld for SquareMaterials {
        fn from_world(world: &mut World) -> Self {
            let mut materials = world.get_resource_mut::<Assets<StandardMaterial>>().unwrap();
            SquareMaterials {
                highlight_color:  materials.add(Color::srgb(1.0, 0.84, 0.0)),
                selected_color:  materials.add(Color::srgb(0.0, 0.0, 1.0)),
                black_color:  materials.add(Color::WHITE),
                white_color: materials.add(Color::BLACK),
            }
        }
    }

// Board Components

    #[derive(Default)]
    pub struct SelectedSquare {
        entity: Option<Entity>,
    }

    pub fn create_board(
        mut commands: Commands,
        mut meshes: ResMut<Assets<Mesh>>, 
        materials: Res<SquareMaterials>,
    ) { 
        let board = meshes.add(Plane3d::default().mesh().size(1.0, 1.0));

        for i in 0..8 {
            for j in 0..8 {
                let material = if (i + j) % 2 == 0 {
                    materials.white_color.clone()
                } else {
                    materials.black_color.clone()
                };            
                commands.spawn((
                    Mesh3d(board.clone()),
                    MeshMaterial3d(material),
                    Transform::from_translation(Vec3::new(i as f32, 0., j as f32)),
                    Board,
                    Square { x: i, y: j },
                ));
        }
    }  

        pub fn update_material_on<E>(
        new_material: Handle<StandardMaterial>,
    )-> impl Fn(Trigger<E>, Query<&mut MeshMaterial3d<StandardMaterial>>) {
        move |trigger, mut query| {
            if let Ok(mut material) = query.get_mut(trigger.entity()) {
                material.0 = new_material.clone();
            }
        }
    }

    pub fn draw_mesh_intersections(pointers: Query<&PointerInteraction>, mut gizmos: Gizmos) {
        for (point, normal) in pointers
        .iter()
        .filter_map(|interaction| interaction.get_nearest_hit())
        .filter_map(|(_entity, hit)| hit.position.zip(hit.normal))
        {
            gizmos.sphere(point, 0.05, RED_500);
            gizmos.arrow(point, point + normal.normalize() * 0.5, PINK_100);
        }
    }   
}







        
   

