    use bevy::prelude::*;
    use crate::pointer_events::{update_squarematl_on, revert_squarematl_on};
    use crate::board_utils::{Square, SquareMaterials, ReturnMaterials};

    #[derive(Resource, Component)]
    pub struct Board;

    pub fn create_board(
        mut commands: Commands,
        mut meshes: ResMut<Assets<Mesh>>, 
        materials: Res<SquareMaterials>,
        return_materials: Res<ReturnMaterials>,
    ) { 
        let boardmesh = meshes.add(Plane3d::default().mesh().size(1.0, 1.0));
        for i in 0..8 {
            for j in 0..8 {
                let square = Square { x: i, y: j };
                let material = if (i + j) % 2 == 0 {
                    materials.white_color.clone()
                } else {
                    materials.black_color.clone()
                };           
                commands.spawn((
                    Mesh3d(boardmesh.clone()),
                    MeshMaterial3d(material.clone()),
                    Transform::from_translation(Vec3::new(i as f32, 0., j as f32)),
                    Square { x: i, y: j },
                    Board,
                ))
                // utilites for changing entity color 
                .observe(update_squarematl_on::<Pointer<Over>>(materials.hover_matl.clone()))
                .observe(revert_squarematl_on::<Pointer<Out>>(return_materials.get_original_material(&square, &materials)));
                }   
            }
        }

    pub struct BoardPlugin;
        impl Plugin for BoardPlugin {
            fn build(&self, app: &mut App) {
                    app.add_systems(Startup, create_board);
            }
        }
                
                


