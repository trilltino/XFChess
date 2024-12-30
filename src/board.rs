    use bevy::{color::palettes::tailwind::*, picking::pointer::{PointerInteraction, PointerId}, prelude::*};


    
        #[derive(Component, Resource)]
    pub struct Board;


     #[derive(Resource, Component, Debug, Clone, Eq, PartialEq)]
        pub struct Square {
            pub x: u8,
            pub y: u8,
        }

        impl Square {
            fn is_white(&self) -> bool {
                (self.x + self.y + 1) % 2 == 0
            }
        }

    #[derive(Resource, Component)]
    pub struct SquareMaterials {
            black_color: Handle<StandardMaterial>,
            white_color: Handle<StandardMaterial>,
            hover_matl :  Handle<StandardMaterial>,
            pressed_matl: Handle<StandardMaterial>,
        }
        
        
        impl FromWorld for SquareMaterials {
            fn from_world(world: &mut World) -> Self {
                let mut materials = world.get_resource_mut::<Assets<StandardMaterial>>().unwrap();
                SquareMaterials {
                    black_color: materials.add(Color::WHITE),
                    white_color: materials.add(Color::BLACK),
                    hover_matl : materials.add(Color::from(CYAN_300)),
                    pressed_matl :materials.add(Color::from(YELLOW_300)),
                }
            }
        }

    // Board Components

        #[derive(Default, Resource, Component)]
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
    }








        
   

