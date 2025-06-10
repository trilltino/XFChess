use bevy::color::palettes::tailwind::*;
use bevy::prelude::*;
use crate::pieces::{Piece,PieceColor};



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



    #[derive(Default, Resource, Component, Debug, Clone, Eq, PartialEq)]
     pub struct BoardEntity {
            pub x: u8,
            pub y: u8,
        }

        impl BoardEntity {
            fn is_white(&self) -> bool {
                (self.x + self.y + 1) % 2 == 0
            }
        }
        

        
        #[derive(Debug, Clone, Resource, Component, Asset, Reflect)]
            pub struct PieceMaterials {
                pub hover_entmatl: Handle<StandardMaterial>,
                pub white_material: Handle<StandardMaterial>,
                pub black_material: Handle<StandardMaterial>,
            }
  

        #[derive(Debug, Clone, Resource)]
        pub struct ReturnEntityMaterials;

        impl Default for ReturnEntityMaterials {
            fn default() -> Self {
                ReturnEntityMaterials
            }
        }

        impl ReturnEntityMaterials {
            pub fn get_original_entmaterial(&self, piece: &Piece, materials: &PieceMaterials) -> Handle<StandardMaterial> {
                if piece.color == PieceColor::White {
                    materials.white_material.clone()
                } else {
                    materials.black_material.clone()
                }
            }
        }


        impl FromWorld for PieceMaterials {
                fn from_world(world: &mut World) -> Self {
                    let mut materials = world.get_resource_mut::<Assets<StandardMaterial>>().unwrap();
                    PieceMaterials {
                    hover_entmatl : materials.add(Color::from(GREEN_300)),
                    white_material : materials.add(Color::from(AMBER_400)),
                    black_material : materials.add(Color::BLACK),
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
                        app.init_resource::<ReturnEntityMaterials>();
                        app.init_resource::<ReturnMaterials>();
                    }
                }
            

                
            
                