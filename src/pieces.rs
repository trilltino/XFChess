    use bevy::{color::palettes::tailwind::*, prelude::*};
    use crate::update_material_on;

    
    
    // add piece types
    // add movement per piece type


    pub fn create_pieces(
        commands: &mut Commands,
        asset_server: Res<AssetServer>,
        mut materials: ResMut<Assets<StandardMaterial>>,
    ) {
        let king_handle: Handle<Mesh> =
            asset_server.load("models/chess_kit/pieces.glb#Mesh0/Primitive0");
        let king_cross_handle: Handle<Mesh> =
            asset_server.load("models/chess_kit/pieces.glb#Mesh1/Primitive0");
        let pawn_handle: Handle<Mesh> =
            asset_server.load("models/chess_kit/pieces.glb#Mesh2/Primitive0");
        let knight_1_handle: Handle<Mesh> =
            asset_server.load("models/chess_kit/pieces.glb#Mesh3/Primitive0");
        let knight_2_handle: Handle<Mesh> =
            asset_server.load("models/chess_kit/pieces.glb#Mesh4/Primitive0");
        let rook_handle: Handle<Mesh> =
            asset_server.load("models/chess_kit/pieces.glb#Mesh5/Primitive0");
        let bishop_handle: Handle<Mesh> =
            asset_server.load("models/chess_kit/pieces.glb#Mesh6/Primitive0");
        let queen_handle: Handle<Mesh> =
            asset_server.load("models/chess_kit/pieces.glb#Mesh7/Primitive0");

            let hover_matl = materials.add(Color::from(GREEN_300));
            let pressed_matl = materials.add(Color::from(BLUE_300));

            
            let white_material = materials.add(StandardMaterial {
                base_color: Color::srgb(1.0, 0.8, 0.8),
                ..Default::default()
            });
            let black_material = materials.add(StandardMaterial {
                base_color: Color::srgb(0.0, 0.2, 0.2),
                ..Default::default()
            });
            
        spawn_rook(
            commands,
            rook_handle.clone(),
            white_material.clone(),
            Vec3::new(0., 0., 0.),
            hover_matl.clone(),
            pressed_matl.clone()
        );


        spawn_knight(
            commands,
            knight_1_handle.clone(),
            knight_2_handle.clone(),
            white_material.clone(),
            Vec3::new(0., 0., 1.),
            hover_matl.clone(),
            pressed_matl.clone()
        );

        spawn_knight(
            commands,
            knight_1_handle.clone(),
            knight_2_handle.clone(),
            white_material.clone(),
            Vec3::new(0., 0., 6.),
            hover_matl.clone(),
            pressed_matl.clone()
        );

        spawn_bishop(
            commands,
            white_material.clone(),
            bishop_handle.clone(),
            Vec3::new(0., 0., 2.),
            hover_matl.clone(),
            pressed_matl.clone()
        );

        spawn_queen(
            commands,
            queen_handle.clone(),
            white_material.clone(),
            Vec3::new(0., 0., 3.),
            hover_matl.clone(),
            pressed_matl.clone()
        );

        spawn_king(
            commands,
            white_material.clone(),
            king_handle.clone(),
            king_cross_handle.clone(),
            Vec3::new(0., 0., 4.),
            hover_matl.clone(),
            pressed_matl.clone(),
        );

        spawn_bishop(
            commands,
            white_material.clone(),
            bishop_handle.clone(),
            Vec3::new(0., 0., 5.),
            hover_matl.clone(),
            pressed_matl.clone()
        );

        spawn_knight(
            commands,
            knight_1_handle.clone(),
            knight_2_handle.clone(),
            white_material.clone(),
            Vec3::new(0., 0., 6.),
            hover_matl.clone(),
            pressed_matl.clone()
        );

        spawn_rook(
            commands,
            rook_handle.clone(),
            white_material.clone(),
            Vec3::new(0., 0., 7.),
            hover_matl.clone(),
            pressed_matl.clone()
        );
    
        for i in 0..8 {
            spawn_pawn(
                commands,
                pawn_handle.clone(),
                white_material.clone(),
                Vec3::new(1., 0., i as f32),
                hover_matl.clone(),
                pressed_matl.clone()
            );
        }

        spawn_rook(
            commands,
            rook_handle.clone(),
            black_material.clone(),
            Vec3::new(7., 0., 0.),
            hover_matl.clone(),
            pressed_matl.clone()
        );
        spawn_knight(
            commands,
            knight_1_handle.clone(),
            knight_2_handle.clone(),
            black_material.clone(),
            Vec3::new(7., 0., 1.),
            hover_matl.clone(),
            pressed_matl.clone()
        );
        spawn_bishop(
            commands,
            black_material.clone(),
            bishop_handle.clone(),
            Vec3::new(7., 0., 2.),
            hover_matl.clone(),
            pressed_matl.clone()
        );
        spawn_queen(
            commands,
            queen_handle.clone(),
            black_material.clone(),
            Vec3::new(7., 0., 3.),
            hover_matl.clone(),
            pressed_matl.clone()
        );
        
        spawn_king(
            commands,
            black_material.clone(),
            king_handle.clone(),
            king_cross_handle.clone(),
            Vec3::new(7., 0., 4.),
            hover_matl.clone(),
            pressed_matl.clone()    
        );
        

        spawn_bishop(
            commands,
            black_material.clone(),
            bishop_handle.clone(),
            Vec3::new(7., 0., 5.),
            hover_matl.clone(),
            pressed_matl.clone()
        );
        spawn_knight(
            commands,
            knight_1_handle.clone(),
            knight_2_handle.clone(),
            black_material.clone(),
            Vec3::new(7., 0., 6.),
            hover_matl.clone(),
            pressed_matl.clone()
        );
        
        spawn_rook(
            commands,
            rook_handle.clone(),
            black_material.clone(),
            Vec3::new(7., 0., 7.),
            hover_matl.clone(),
            pressed_matl.clone()
        );

        for i in 0..8 {
            spawn_pawn(
                commands,
                pawn_handle.clone(),
                black_material.clone(),
                Vec3::new(6., 0., i as f32),
                hover_matl.clone(),
                pressed_matl.clone()
            );
        }
            pub fn spawn_king(
                commands: &mut Commands,
                material: Handle<StandardMaterial>,
                mesh: Handle<Mesh>,
                mesh_cross: Handle<Mesh>,
                position: Vec3,
                hover_matl: Handle<StandardMaterial>,
                pressed_matl: Handle<StandardMaterial>
            ) {

                commands.spawn((
                    Transform::from_translation(position),
                ))
                .with_children(|parent| {
                    parent.spawn((
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(material.clone()),
                        {
                            let mut transform = Transform::from_translation(Vec3::new(-0.2, 0., -1.9));
                            transform.scale = Vec3::new(0.2, 0.2, 0.2);
                            transform
                        },
                    ))
                    .observe(update_material_on::<Pointer<Over>>(hover_matl.clone()))
                    .observe(update_material_on::<Pointer<Click>>(pressed_matl.clone()));
                    parent.spawn((
                        Mesh3d(mesh_cross),
                        MeshMaterial3d(material),
                        {
                            let mut transform = Transform::from_translation(Vec3::new(-0.2, 0., -1.9));
                            transform.scale = Vec3::new(0.2, 0.2, 0.2);
                            transform
                        },
                    ))
                    .observe(update_material_on::<Pointer<Over>>(hover_matl.clone()))
                    .observe(update_material_on::<Pointer<Click>>(pressed_matl.clone()));
                });
            }
                
            pub fn spawn_knight (
                commands: &mut Commands,
                mesh_1: Handle<Mesh>,
                mesh_2: Handle<Mesh>,
                material: Handle<StandardMaterial>,
                position: Vec3,        
                hover_matl: Handle<StandardMaterial>,
                pressed_matl: Handle<StandardMaterial>
        ) {
            commands.spawn((
                Transform::from_translation(position),
            ))
            .with_children(|parent| {
                parent.spawn((
                    Mesh3d(mesh_1.clone()),
                    MeshMaterial3d(material.clone()),
                    {
                        let mut transform = Transform::from_translation(Vec3::new(-0.2, 0., 0.9));
                        transform.scale = Vec3::new(0.2, 0.2, 0.2);
                        transform
                    },
                ))
                .observe(update_material_on::<Pointer<Over>>(hover_matl.clone()))
                .observe(update_material_on::<Pointer<Click>>(pressed_matl.clone()));
                parent.spawn((
                    Mesh3d(mesh_2.clone()),
                    MeshMaterial3d(material),
                    {
                        let mut transform = Transform::from_translation(Vec3::new(-0.2, 0., 0.9));
                        transform.scale = Vec3::new(0.2, 0.2, 0.2);
                        transform
                    },
                ))
                .observe(update_material_on::<Pointer<Over>>(hover_matl.clone()))
                .observe(update_material_on::<Pointer<Click>>(pressed_matl.clone()));
            });
        }

        pub fn spawn_queen (
            commands: &mut Commands,
            mesh: Handle<Mesh>,
            material: Handle<StandardMaterial>,
            position: Vec3,
            hover_matl: Handle<StandardMaterial>,
            pressed_matl: Handle<StandardMaterial>        
        ) {
        commands.spawn((
            Transform::from_translation(position),
        ))
        .with_children(|parent| {
            parent.spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material.clone()),
                {
                    let mut transform = Transform::from_translation(Vec3::new(-0.2, 0., -0.95));
                    transform.scale = Vec3::new(0.2, 0.2, 0.2);
                    transform
                },
            ))
            .observe(update_material_on::<Pointer<Over>>(hover_matl.clone()))
            .observe(update_material_on::<Pointer<Click>>(pressed_matl.clone()));
        });
    }

    
            pub fn spawn_bishop(
                commands: &mut Commands,
                material: Handle<StandardMaterial>,
                mesh: Handle<Mesh>,
                position: Vec3,
                hover_matl: Handle<StandardMaterial>,
                pressed_matl: Handle<StandardMaterial>
            ) {
                commands.spawn((
                    Transform::from_translation(position),
                ))
                .with_children(|parent| {
                    parent.spawn((
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(material.clone()),
                        {
                            let mut transform = Transform::from_translation(Vec3::new(-0.1, 0., 0.0));
                            transform.scale = Vec3::new(0.2, 0.2, 0.2);
                            transform
                        },
                    ))
                    .observe(update_material_on::<Pointer<Over>>(hover_matl.clone()))
                    .observe(update_material_on::<Pointer<Click>>(pressed_matl.clone()));
                });
            }

            pub fn spawn_rook (
                commands: &mut Commands,
                mesh: Handle<Mesh>,
                material: Handle<StandardMaterial>,
                position: Vec3,   
                hover_matl: Handle<StandardMaterial>,
                pressed_matl: Handle<StandardMaterial>     
        ) {
            commands.spawn((
                Transform::from_translation(position),
            ))
            .with_children(|parent| {
                parent.spawn((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material.clone()),
                    {
                        let mut transform = Transform::from_translation(Vec3::new(-0.1, 0., 1.8));
                        transform.scale = Vec3::new(0.2, 0.2, 0.2);
                        transform
                    },
                ))
                .observe(update_material_on::<Pointer<Over>>(hover_matl.clone()))
                .observe(update_material_on::<Pointer<Click>>(pressed_matl.clone()));
            });
        }
    

            pub fn spawn_pawn (
                    commands: &mut Commands,
                    mesh: Handle<Mesh>,
                    material: Handle<StandardMaterial>,
                    position: Vec3,   
                    hover_matl: Handle<StandardMaterial>,
                    pressed_matl: Handle<StandardMaterial>     
            ) {
                commands.spawn((
                    Transform::from_translation(position),
                ))
                .with_children(|parent| {
                    parent.spawn((
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(material.clone()),
                        {
                            let mut transform = Transform::from_translation(Vec3::new(-0.2, 0., 2.6));
                            transform.scale = Vec3::new(0.2, 0.2, 0.2);
                            transform
                        },
                    ))
                    .observe(update_material_on::<Pointer<Over>>(hover_matl.clone()))
                    .observe(update_material_on::<Pointer<Click>>(pressed_matl.clone()));
                });
            }
        }




            


