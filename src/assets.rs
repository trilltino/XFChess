use bevy::prelude::*;


#[derive(Clone, Copy, PartialEq,)]
pub enum PieceColor {
    White,
    Black,
}

#[derive(Clone, Copy, PartialEq)]
pub enum PieceType {
    King,
    Queen,
    Bishop,
    Knight,
    Rook,
    Pawn,
}

#[derive(Clone, Copy, Component)]
pub struct Piece {
    pub color: PieceColor,
    pub piece_type: PieceType,
    pub x: f32,
    pub y: f32,
}

pub fn move_pieces(time: Res<Time>, mut query: Query<(&mut Transform, &Piece)>) {
    for (mut transform, piece) in query.iter_mut(){
        let direction = Vec3::new(piece.x, 0., piece.y) - transform.translation;
        if direction.length() > 0.1 {
            transform.translation += direction.normalize() * time.delta_seconds();
        }
    }
}

pub fn create_pieces(
    commands: &mut Commands,
     server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {

    let king_handle: Handle<Mesh> =
        server.load("models/chess_kit/pieces.glb#Mesh0/Primitive0");
    let king_cross_handle: Handle<Mesh> =
        server.load("models/chess_kit/pieces.glb#Mesh1/Primitive0");
    let pawn_handle: Handle<Mesh> =
        server.load("models/chess_kit/pieces.glb#Mesh2/Primitive0");
    let knight_1_handle: Handle<Mesh> =
        server.load("models/chess_kit/pieces.glb#Mesh3/Primitive0");
    let knight_2_handle: Handle<Mesh> =
        server.load("models/chess_kit/pieces.glb#Mesh4/Primitive0");
    let rook_handle: Handle<Mesh> =
        server.load("models/chess_kit/pieces.glb#Mesh5/Primitive0");
    let bishop_handle: Handle<Mesh> =
        server.load("models/chess_kit/pieces.glb#Mesh6/Primitive0");
    let queen_handle: Handle<Mesh> =
        server.load("models/chess_kit/pieces.glb#Mesh7/Primitive0");

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
            white_material.clone(),
            PieceColor::White,
            rook_handle.clone(),
            Vec3::new(0., 0., 0.),
        );

        spawn_knight(
            commands,
            white_material.clone(),
            PieceColor::White,
            knight_1_handle.clone(),
            knight_2_handle.clone(),
            Vec3::new(0., 0., 1.),
        );
        spawn_bishop(
            commands,
            white_material.clone(),
            PieceColor::White,
            bishop_handle.clone(),
            Vec3::new(0., 0., 2.),
        );
        spawn_queen(
            commands,
            white_material.clone(),
            PieceColor::White,
            queen_handle.clone(),
            Vec3::new(0., 0., 3.),
        );
        spawn_king(
            commands,
            white_material.clone(),
            PieceColor::White,
            king_handle.clone(),
            king_cross_handle.clone(),
            Vec3::new(0., 0., 4.),
        );
        spawn_bishop(
            commands,
            white_material.clone(),
            PieceColor::White,
            bishop_handle.clone(),
            Vec3::new(0., 0., 5.),
        );
        spawn_knight(
            commands,
            white_material.clone(),
            PieceColor::White,
            knight_1_handle.clone(),
            knight_2_handle.clone(),
            Vec3::new(0., 0., 6.),
        );
        spawn_rook(
            commands,
            white_material.clone(),
            PieceColor::White,
            rook_handle.clone(),
            Vec3::new(0., 0., 7.),
        );
    
        for i in 0..8 {
            spawn_pawn(
                commands,
                white_material.clone(),
                PieceColor::White,
                pawn_handle.clone(),
                Vec3::new(1., 0., i as f32),
            );
        }
    
        spawn_rook(
            commands,
            black_material.clone(),
            PieceColor::Black,
            rook_handle.clone(),
            Vec3::new(7., 0., 0.),
        );
        spawn_knight(
            commands,
            black_material.clone(),
            PieceColor::Black,
            knight_1_handle.clone(),
            knight_2_handle.clone(),
            Vec3::new(7., 0., 1.),
        );
        spawn_bishop(
            commands,
            black_material.clone(),
            PieceColor::Black,
            bishop_handle.clone(),
            Vec3::new(7., 0., 2.),
        );
        spawn_queen(
            commands,
            black_material.clone(),
            PieceColor::Black,
            queen_handle.clone(),
            Vec3::new(7., 0., 3.),
        );
        spawn_king(
            commands,
            black_material.clone(),
            PieceColor::Black,
            king_handle.clone(),
            king_cross_handle.clone(),
            Vec3::new(7., 0., 4.),
        );
        spawn_bishop(
            commands,
            black_material.clone(),
            PieceColor::Black,
            bishop_handle.clone(),
            Vec3::new(7., 0., 5.),
        );
        spawn_knight(
            commands,
            black_material.clone(),
            PieceColor::Black,
            knight_1_handle.clone(),
            knight_2_handle.clone(),
            Vec3::new(7., 0., 6.),
        );
        spawn_rook(
            commands,
            black_material.clone(),
            PieceColor::Black,
            rook_handle.clone(),
            Vec3::new(7., 0., 7.),
        );
    
        for i in 0..8 {
            spawn_pawn(
                commands,
                black_material.clone(),
                PieceColor::Black,
                pawn_handle.clone(),
                Vec3::new(6., 0., i as f32),
            );
        }
}


        pub fn spawn_king(
            commands: &mut Commands,
            material: Handle<StandardMaterial>,
            piece_color: PieceColor,
            mesh: Handle<Mesh>,
            mesh_cross: Handle<Mesh>,
            position: Vec3,
        ) {
            commands
                .spawn(PbrBundle {
                    transform: Transform::from_translation(position),
                    ..Default::default()
                })

                .insert(Piece {
                    color: piece_color,
                    piece_type: PieceType::King,
                    x: position.x as f32,
                    y: position.z as f32,
                })

                .with_children(|parent| {
                    parent.spawn(PbrBundle {
                        mesh,
                        material: material.clone(),
                        transform: {
                            let mut transform = Transform::from_translation(Vec3::new(-0.25, 0., -1.9));
                            transform.scale = Vec3::new(0.2, 0.2, 0.2);
                            transform
                        },
                        ..Default::default()
                    });
                    parent.spawn(PbrBundle {
                        mesh: mesh_cross,
                        material,
                        transform: {
                            let mut transform = Transform::from_translation(Vec3::new(-0.25, 0., -1.9));
                            transform.scale = Vec3::new(0.2, 0.2, 0.2);
                            transform
                        },
                        ..Default::default()
                    });
                });
        }

pub fn spawn_knight(
    commands: &mut Commands,
    material: Handle<StandardMaterial>,
    piece_color: PieceColor,
    mesh_1: Handle<Mesh>,
    mesh_2: Handle<Mesh>,
    position: Vec3,
) {
    commands.spawn(PbrBundle {
        transform: Transform::from_translation(position),
        ..Default::default()
    })
    .insert(Piece {
        color: piece_color,
        piece_type: PieceType::Knight,
        x: position.x as f32,
        y: position.z as f32,
    })
    .with_children(|parent|{
        parent.spawn(PbrBundle {
            mesh: mesh_1,
            material: material.clone(),
            transform: {
                let mut transform = Transform::from_translation(Vec3::new(-0.2, 0., 1.0));
                transform.scale = Vec3::new(0.2, 0.2, 0.2);
                transform
            },
            ..Default::default()
        });
        parent.spawn(PbrBundle{
            mesh: mesh_2,
            material,
            transform: {
                let mut transform = Transform::from_translation(Vec3::new(-0.2, 0., 1.0));
                transform.scale = Vec3::new(0.2, 0.2, 0.2);
                transform
            },
            ..Default::default()
        });
    });
}

pub fn spawn_queen(
    commands: &mut Commands,
    material: Handle<StandardMaterial>,
    piece_color: PieceColor,
    mesh: Handle<Mesh>,
    position: Vec3,
) {
    commands
        .spawn(PbrBundle {
            transform: Transform::from_translation(position),
            ..Default::default()
        })
        .insert(Piece {
            color: piece_color,
            piece_type: PieceType::Queen,
            x: position.x as f32,
            y: position.z as f32,
        })
        .with_children(|parent| {
            parent.spawn(PbrBundle {
                mesh,
                material,
                transform: {
                    let mut transform = Transform::from_translation(Vec3::new(-0.2, 0., -0.95));
                    transform.scale = Vec3::new(0.2, 0.2, 0.2);
                    transform
                },
                ..Default::default()
            });
        });
}

pub fn spawn_bishop(
    commands: &mut Commands,
    material: Handle<StandardMaterial>,
    piece_color: PieceColor,
    mesh: Handle<Mesh>,
    position: Vec3,
) {
    commands
        .spawn(PbrBundle {
            transform: Transform::from_translation(position),
            ..Default::default()
        })
        .insert(Piece {
            color: piece_color,
            piece_type: PieceType::Bishop,
            x: position.x as f32,
            y: position.z as f32,
        })
        .with_children(|parent| {
            parent.spawn(PbrBundle {
                mesh,
                material,
                transform: {
                    let mut transform = Transform::from_translation(Vec3::new(-0.1, 0., 0.));
                    transform.scale = Vec3::new(0.2, 0.2, 0.2);
                    transform
                },
                ..Default::default()
            });
        });
}

pub fn spawn_rook(
    commands: &mut Commands,
    material: Handle<StandardMaterial>,
    piece_color: PieceColor,
    mesh: Handle<Mesh>,
    position: Vec3,
) {
    commands
        .spawn(PbrBundle {
            transform: Transform::from_translation(position),
            ..Default::default()
        })
        .insert(Piece {
            color: piece_color,
            piece_type: PieceType::Rook,
            x: position.x as f32,
            y: position.z as f32,
        })
        .with_children(|parent| {
            parent.spawn(PbrBundle {
                mesh,
                material,
                transform: {
                    let mut transform = Transform::from_translation(Vec3::new(-0.1, 0., 1.8));
                    transform.scale = Vec3::new(0.2, 0.2, 0.2);
                    transform
                },
                ..Default::default()
            });
        });
}

pub fn spawn_pawn(
    commands: &mut Commands,
    material: Handle<StandardMaterial>,
    piece_color: PieceColor,
    mesh: Handle<Mesh>,
    position: Vec3,
) {
    commands
        .spawn(PbrBundle {
            transform: Transform::from_translation(position),
            ..Default::default()
        })
        .insert(Piece {
            color: piece_color,
            piece_type: PieceType::Pawn,
            x: position.x as f32,
            y: position.z as f32,
        })
        .with_children(|parent| {
            parent.spawn(PbrBundle {
                mesh,
                material,
                transform: {
                    let mut transform = Transform::from_translation(Vec3::new(-0.1, 0., 2.5));
                    transform.scale = Vec3::new(0.2, 0.2, 0.2);
                    transform
                },
                ..Default::default()
            });
        });
}

pub struct PiecesPlugin;
impl Plugin for PiecesPlugin {
    fn build(&self, app: &mut App) {
        
        app.add_systems(Startup, |mut commands: Commands, server: Res<AssetServer>, materials: ResMut<Assets<StandardMaterial>>| {
            create_pieces(&mut commands, server, materials);
        })
        .add_systems(Update, move_pieces);
    }
}
