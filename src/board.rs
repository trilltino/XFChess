    use bevy::{color::palettes::tailwind::*, prelude::*};
    use crate::pointer_events::{update_squarematl_on, revert_squarematl_on};
    

    #[derive(Component, Resource)]
    pub struct Board;


// Sqaure Implementations
    
    #[derive(Default, Resource)]
    struct SelectedSquare {
        entity: Option<Entity>,
    }
   
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
            hover_matl: Handle<StandardMaterial>,
            clicked_matl: Handle<StandardMaterial>, 
        }


        #[derive(Debug, Resource)]

        pub struct ReturnMaterials;

        impl Default for ReturnMaterials {
            fn default() -> Self {
                ReturnMaterials
            }
        }

        impl ReturnMaterials {
            fn get_original_material(&self, square: &Square, materials: &SquareMaterials) -> Handle<StandardMaterial> {
                if square.is_white() {
                    materials.white_color.clone()
                } else {
                    materials.black_color.clone()
                }
            }
        }
        
          
        impl FromWorld for SquareMaterials {
            fn from_world(world: &mut World) -> Self {
                let mut materials = world.get_resource_mut::<Assets<StandardMaterial>>().unwrap();
                SquareMaterials {
                    black_color: materials.add(Color::WHITE),
                    white_color: materials.add(Color::BLACK),
                    hover_matl : materials.add(Color::from(AMBER_100)),
                    clicked_matl : materials.add(Color::from(RED_300)),

                }
            }
        }



    // Board functions

      pub fn create_board(
            mut commands: Commands,
            mut meshes: ResMut<Assets<Mesh>>, 
            materials: Res<SquareMaterials>,
            return_materials: Res<ReturnMaterials>,
        ) { 
        
            let board = meshes.add(Plane3d::default().mesh().size(1.0, 1.0));

 
            for i in 0..8 {
                for j in 0..8 {
                    let square = Square { x: i, y: j };
                    let material = if (i + j) % 2 == 0 {
                        materials.white_color.clone()
                    } else {
                        materials.black_color.clone()
                    };            
                    commands.spawn((
                        Mesh3d(board.clone()),
                        MeshMaterial3d(material.clone()),
                        Transform::from_translation(Vec3::new(i as f32, 0., j as f32)),
                        Square { x: i, y: j },
                        Board,
                    ))
                    .observe(update_squarematl_on::<Pointer<Over>>(materials.hover_matl.clone()))
                    .observe(update_squarematl_on::<Pointer<Down>>(materials.clicked_matl.clone()))
                    .observe(revert_squarematl_on::<Pointer<Out>>(return_materials.get_original_material(&square, &materials)));
                  }   
                }
            }
        
        pub struct BoardPlugin;
            impl Plugin for BoardPlugin {
                fn build(&self, app: &mut App) {
                   app.init_resource::<SelectedSquare>();
                    app.init_resource::<SquareMaterials>()
                        .init_resource::<ReturnMaterials>()
                        .add_systems(Startup, create_board);
                }
            }
        


        
   
        

