    use bevy::color::palettes::tailwind::*;
    use bevy::prelude::*;

    #[derive(Default, Resource, Component, Debug, Clone, Eq, PartialEq)]
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
            pub black_color: Handle<StandardMaterial>,
            pub white_color: Handle<StandardMaterial>,
            pub hover_matl:  Handle<StandardMaterial>,
        }

    impl FromWorld for SquareMaterials {
        fn from_world(world: &mut World) -> Self {
            let mut materials = world.get_resource_mut::<Assets<StandardMaterial>>().unwrap();
            SquareMaterials {
                black_color: materials.add(Color::WHITE),
                white_color: materials.add(Color::BLACK),
                hover_matl : materials.add(Color::from(AMBER_100)),
            }
        }
    }
  
    #[derive(Debug, Resource)]
    pub struct ReturnMaterials;

    impl Default for ReturnMaterials {
        fn default() -> Self {
            ReturnMaterials
        }
    }

    impl ReturnMaterials {
        pub fn get_original_material(&self, square: &Square, materials: &SquareMaterials) -> Handle<StandardMaterial> {
            if square.is_white() {
                materials.black_color.clone()
            } else {
                materials.white_color.clone()
            }
        }
    }

    pub struct BoardUtils;
            impl Plugin for BoardUtils {
                fn build(&self, app: &mut App) {
                    app.init_resource::<SquareMaterials>();
                    app.init_resource::<ReturnMaterials>();
                }
            }
            

                
            
                