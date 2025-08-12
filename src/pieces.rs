    use bevy::prelude::*;
    use bevy::picking::pointer::PointerInteraction;
    use std::f32;
    use bevy::color::Color;

    #[derive(Clone, Copy, Debug, Component, PartialEq)]
    pub enum PieceColor {
        White,
        Black,
    }

    #[derive(Component,Clone, Copy, PartialEq, Debug)]
    pub enum PieceType {
        King,
        Queen,
        Bishop,
        Knight,
        Rook,
        Pawn,
    }

    #[derive(Component,Clone, Debug, Copy)]
        pub struct Piece {
            pub color: PieceColor,
            pub piece_type: PieceType,
            pub x: u8,
            pub y: u8,
            
        }

     
    fn create_pieces(
        mut commands: Commands,
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

            
            
        let white_material = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            ..default()
        });
        let black_material = materials.add(StandardMaterial {
            base_color: Color::BLACK,
            ..default()
        });           
                
        spawn_rook(
            &mut commands,
            white_material.clone(),
            PieceColor::White,
            rook_handle.clone(),
            (0, 0),
        );

        spawn_knight(
            &mut commands,
            white_material.clone(),
            PieceColor::White,
            knight_1_handle.clone(),
            knight_2_handle.clone(),
            (0, 1),
        );


        spawn_bishop(
            &mut commands,
            white_material.clone(),
            PieceColor::White,
            bishop_handle.clone(),
            (0, 2),
        );

        spawn_queen(
            &mut commands,
            white_material.clone(),
            PieceColor::White,
            queen_handle.clone(),
            (0, 3),
        );

        spawn_king(
            &mut commands,
            white_material.clone(), 
            PieceColor::White,
            king_handle.clone(),
            king_cross_handle.clone(),
            (0, 4),
        );

        spawn_bishop(
            &mut commands,
            white_material.clone(),
            PieceColor::White,
            bishop_handle.clone(),
            (0, 5),
        );

        spawn_knight(
            &mut commands,
            white_material.clone(),
            PieceColor::White,
            knight_1_handle.clone(),
            knight_2_handle.clone(),
            (0, 6),
        );

        spawn_rook(
            &mut commands,
            white_material.clone(),
            PieceColor::White,
            rook_handle.clone(),
            (0, 7),
        );
    
        for i in 0..8 {
            spawn_pawn(
            &mut commands,
            white_material.clone(),
            PieceColor::White,
            pawn_handle.clone(),
            (1, i),
            );
        }

        spawn_rook(
            &mut commands,
            black_material.clone(),
            PieceColor::Black,
            rook_handle.clone(),
            (7, 0),
        );
        spawn_knight(
            &mut commands,
            black_material.clone(),
            PieceColor::Black,
            knight_1_handle.clone(),
            knight_2_handle.clone(),
            (7, 1),
        );

        spawn_bishop(
            &mut commands,
            black_material.clone(),
            PieceColor::Black,
            bishop_handle.clone(),
            (7, 2),
        );

        spawn_queen(
            &mut commands,
            black_material.clone(),
            PieceColor::Black,
            queen_handle.clone(),
            (7, 3),
        );
        
        spawn_king(
            &mut commands,
            black_material.clone(),
            PieceColor::Black,
            king_handle.clone(),
            king_cross_handle.clone(),    
            (7, 4),
        );
        
        spawn_bishop(
            &mut commands,
            black_material.clone(),
            PieceColor::Black,
            bishop_handle.clone(),
            (7, 5),
        );
        spawn_knight(
            &mut commands,
            black_material.clone(),
            PieceColor::Black,
            knight_1_handle.clone(),
            knight_2_handle.clone(),
            (7, 6),
        );
        
        spawn_rook(
            &mut commands,
            black_material.clone(),
            PieceColor::Black,
            rook_handle.clone(),
            (7, 7),
        );

        for i in 0..8 {
            spawn_pawn(
            &mut commands,
            black_material.clone(),
            PieceColor::Black,
    pawn_handle.clone(),
(6, i),
            );
        }
    }

    fn piece_transform(offset: Vec3) -> Transform {
        let mut t = Transform::from_translation(offset);
        t.scale = Vec3::splat(0.2);
        t
    }
        #[allow(clippy::too_many_arguments)]    
            pub fn spawn_king(
                commands: &mut Commands,         
                material: Handle<StandardMaterial>,
                piece_color: PieceColor,
                mesh: Handle<Mesh>,
                mesh_cross: Handle<Mesh>,
                position: (u8, u8),
            ) {
            commands
                .spawn(Transform::from_translation(Vec3::new(
                    position.0 as f32,
                    0.,
                    position.1 as f32,
                )))
                .insert(Piece {
                    color: piece_color,
                    piece_type: PieceType::King,
                    x: position.0,
                    y: position.1,
                })
                .insert(PointerInteraction::default())
                .with_children(|parent| {
                    parent
                        .spawn((
                            Mesh3d(mesh.clone()),
                            MeshMaterial3d(material.clone()),
                            piece_transform(Vec3::new(-0.2, 0., -1.9)),
                        ));
                    parent
                        .spawn((
                            Mesh3d(mesh_cross),
                            MeshMaterial3d(material.clone()),
                            piece_transform(Vec3::new(-0.2, 0., -1.9)),
                        ));
                });
}

    pub fn spawn_knight(
        commands: &mut Commands,
        material: Handle<StandardMaterial>,
        piece_color: PieceColor,
        mesh_1: Handle<Mesh>,
        mesh_2: Handle<Mesh>,
        position: (u8, u8),
    ) {
        commands
            .spawn(Transform::from_translation(Vec3::new(position.0 as f32, 0., position.1 as f32)))
            .insert(Piece {
            color: piece_color,
            piece_type: PieceType::Knight,
            x: position.0,
            y: position.1,
        })
            .with_children(|parent| {
                parent
                    .spawn((
                        Mesh3d(mesh_1.clone()),
                        MeshMaterial3d(material.clone()),
                        piece_transform(Vec3::new(-0.2, 0., 0.9)),
                    ));                   
                parent
                    .spawn((
                        Mesh3d(mesh_2),
                        MeshMaterial3d(material),
                        piece_transform(Vec3::new(-0.2, 0., 0.9)),
                    ));                  
            });
    }
        
    pub fn spawn_queen(
        commands: &mut Commands,
        material: Handle<StandardMaterial>,
        piece_color: PieceColor,
        mesh: Handle<Mesh>,
        position: (u8, u8),
    ) {
        commands
            .spawn(Transform::from_translation(Vec3::new(position.0 as f32, 0., position.1 as f32)))
            .insert(Piece {
            color: piece_color,
            piece_type: PieceType::Queen,
            x: position.0,
            y: position.1,
        })
            .with_children(|parent| {
                parent
                    .spawn((
                        Mesh3d(mesh),
                        MeshMaterial3d(material),
                        piece_transform(Vec3::new(-0.2, 0., -0.95)),
                    ));
            });
    }

    pub fn spawn_bishop(
        commands: &mut Commands,
        material: Handle<StandardMaterial>,
        piece_color: PieceColor,
        mesh: Handle<Mesh>,
        position: (u8, u8),
    ) {
        commands
            .spawn(Transform::from_translation(Vec3::new(position.0 as f32, 0., position.1 as f32)))
            .insert(Piece {
            color: piece_color,
            piece_type: PieceType::Bishop,
            x: position.0,
            y: position.1,
        })
            .with_children(|parent| {
                parent
                    .spawn((
                        Mesh3d(mesh),
                        MeshMaterial3d(material),
                        piece_transform(Vec3::new(-0.1, 0., 0.0)),
                    ));
            });
    }

    pub fn spawn_rook(
        commands: &mut Commands,
        material: Handle<StandardMaterial>,
        piece_color: PieceColor,
        mesh: Handle<Mesh>,
        position: (u8, u8),
    ) {
        commands
            .spawn(Transform::from_translation(Vec3::new(position.0 as f32, 0., position.1 as f32)))
            .insert(Piece {
            color: piece_color,
            piece_type: PieceType::Rook,
            x: position.0,
            y: position.1,
        })
            .with_children(|parent| {
                parent
                    .spawn((
                        Mesh3d(mesh),
                        MeshMaterial3d(material),
                        piece_transform(Vec3::new(-0.1, 0., 1.8)),
                    ));
            });
    }

    pub fn spawn_pawn(
        commands: &mut Commands,
        material: Handle<StandardMaterial>,
        piece_color: PieceColor,
        mesh: Handle<Mesh>,
        position: (u8, u8),
    ) {
        commands
            .spawn(Transform::from_translation(Vec3::new(position.0 as f32, 0., position.1 as f32)))
            .insert(Piece {
            color: piece_color,
            piece_type: PieceType::Pawn,
            x: position.0,
            y: position.1,
        })
            .with_children(|parent| {
                parent
                    .spawn((
                        Mesh3d(mesh),
                        MeshMaterial3d(material),
                        piece_transform(Vec3::new(-0.2, 0., 2.6)),
                    ));
            });
    }

    pub struct PiecePlugin;
    impl Plugin for PiecePlugin {
        fn build(&self, app: &mut App) {
            app.add_systems(Startup, create_pieces);
        }
    }

