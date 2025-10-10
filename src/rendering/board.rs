    use bevy::prelude::*;
    // use crate::input::{update_squarematl_on, revert_squarematl_on}; // TODO: Fix observer API
    use crate::rendering::utils::{Square, SquareMaterials};

    #[derive(Resource, Component)]
    pub struct Board;

    pub fn create_board(
        mut commands: Commands,
        mut meshes: ResMut<Assets<Mesh>>,
        materials: Res<SquareMaterials>,
    ) { 
        let boardmesh = meshes.add(Plane3d::default().mesh().size(1.0, 1.0));
        for i in 0..8 {
            for j in 0..8 {
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
                ));
                // TODO: Re-enable observers for hover effects once Bevy 0.17 API is clarified
                // For now, picking events work via the MessageReader in board_events.rs
                // .observe(update_squarematl_on::<Pointer<Over>>(materials.hover_matl.clone()))
                // .observe(revert_squarematl_on::<Pointer<Out>>(return_materials.get_original_material(&square, &materials)));
                }   
            }
        }

    pub struct BoardPlugin;
        impl Plugin for BoardPlugin {
            fn build(&self, app: &mut App) {
                    app.add_systems(Startup, create_board);
            }
        }
                
                


