//! Chess piece 3D rendering - Data-driven GLTF model spawning
//!
//! Implements idiomatic Bevy 0.17 entity spawning for chess pieces using data-driven
//! patterns instead of repetitive manual spawning. Demonstrates modern ECS best practices.
//!
//! # Architecture Improvements
//!
//! **Previous Approach** (Anti-pattern):
//! - 32+ individual spawn calls hardcoded
//! - Repeated code for each piece
//! - Difficult to modify starting positions
//!
//! **Current Approach** (Idiomatic):
//! - `const BACK_ROW` array defines standard chess starting position
//! - Loop-based spawning with `spawn_piece_at` dispatcher
//! - Single source of truth for piece placement
//! - Easy to test and modify
//!
//! # Bevy 0.17 Patterns
//!
//! - `Mesh3d` component for mesh handles
//! - `MeshMaterial3d<StandardMaterial>` for materials
//! - `PointerInteraction` for built-in picking
//! - Component bundles via `.insert()` chains
//! - Reflection support with `#[reflect(Component)]`
//!
//! # GLTF Asset Loading
//!
//! Uses asset path fragments to load individual meshes from a single GLTF file:
//! - `pieces.glb#Mesh0/Primitive0` - King base
//! - `pieces.glb#Mesh1/Primitive0` - King cross
//! - etc.
//!
//! # Stack Overflow Prevention
//!
//! Spawning 32 pieces triggers concurrent GLTF parsing in Bevy's Compute Task Pool.
//! The recursive GLTF node traversal requires >2MB stack (default is 2MB). Fixed by:
//! - `.cargo/config.toml` linker config: 8MB stack for all threads
//! - Asset preloading (Tier 3): Load GLTF before state transition
//! - Pattern matches `reference/bevy/examples/stress_tests/many_foxes.rs` (1000+ GLTF models)
//!
//! # Reference
//!
//! - `reference/bevy/examples/asset/asset_loading.rs` - GLTF loading patterns
//! - `reference/bevy/examples/ecs/iter_combinations.rs` - Data-driven spawning
//! - `reference/bevy/examples/stress_tests/many_foxes.rs` - Mass GLTF spawning
//! - `reference/bevy-3d-chess/` - Alternative piece spawning approach

    use bevy::prelude::*;
    use bevy::picking::pointer::PointerInteraction;
    use std::f32;
    use bevy::color::Color;
    use crate::game::components::HasMoved;
    use crate::game::systems::input::on_piece_click;
    use crate::input::pointer::{on_piece_hover, on_piece_unhover};

    #[derive(Clone, Copy, Debug, Component, PartialEq, Eq, Reflect)]
    #[reflect(Component)]
    pub enum PieceColor {
        White,
        Black,
    }

    #[derive(Component, Clone, Copy, PartialEq, Debug, Reflect)]
    #[reflect(Component)]
    pub enum PieceType {
        King,
        Queen,
        Bishop,
        Knight,
        Rook,
        Pawn,
    }

    #[derive(Component, Clone, Debug, Copy, Reflect)]
    #[reflect(Component)]
        pub struct Piece {
            pub color: PieceColor,
            pub piece_type: PieceType,
            pub x: u8,
            pub y: u8,
        }

    /// Data-driven piece setup - idiomatic Bevy approach
    ///
    /// Uses const arrays to define starting positions, then iterates to spawn pieces.
    /// This pattern is cleaner, more maintainable, and easier to test than manual spawning.
    ///
    /// Reference: `reference/bevy/examples/ecs/` for data-driven entity spawning patterns
    fn create_pieces(
        mut commands: Commands,
        asset_server: Res<AssetServer>,
        mut materials: ResMut<Assets<StandardMaterial>>,
    ) {
        // Load all piece meshes (shared across colors)
        let piece_meshes = PieceMeshes {
            king: asset_server.load("models/chess_kit/pieces.glb#Mesh0/Primitive0"),
            king_cross: asset_server.load("models/chess_kit/pieces.glb#Mesh1/Primitive0"),
            pawn: asset_server.load("models/chess_kit/pieces.glb#Mesh2/Primitive0"),
            knight_1: asset_server.load("models/chess_kit/pieces.glb#Mesh3/Primitive0"),
            knight_2: asset_server.load("models/chess_kit/pieces.glb#Mesh4/Primitive0"),
            rook: asset_server.load("models/chess_kit/pieces.glb#Mesh5/Primitive0"),
            bishop: asset_server.load("models/chess_kit/pieces.glb#Mesh6/Primitive0"),
            queen: asset_server.load("models/chess_kit/pieces.glb#Mesh7/Primitive0"),
        };

        let white_material = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            ..default()
        });
        let black_material = materials.add(StandardMaterial {
            base_color: Color::BLACK,
            ..default()
        });

        // Data-driven piece placement using standard chess starting positions
        const BACK_ROW: [PieceType; 8] = [
            PieceType::Rook, PieceType::Knight, PieceType::Bishop, PieceType::Queen,
            PieceType::King, PieceType::Bishop, PieceType::Knight, PieceType::Rook,
        ];

        // Spawn white pieces
        for (file, &piece_type) in BACK_ROW.iter().enumerate() {
            spawn_piece_at(
                &mut commands,
                &piece_meshes,
                white_material.clone(),
                PieceColor::White,
                piece_type,
                (0, file as u8),
            );
        }

        // Spawn white pawns
        for file in 0..8 {
            spawn_piece_at(
                &mut commands,
                &piece_meshes,
                white_material.clone(),
                PieceColor::White,
                PieceType::Pawn,
                (1, file),
            );
        }

        // Spawn black pieces
        for (file, &piece_type) in BACK_ROW.iter().enumerate() {
            spawn_piece_at(
                &mut commands,
                &piece_meshes,
                black_material.clone(),
                PieceColor::Black,
                piece_type,
                (7, file as u8),
            );
        }

        // Spawn black pawns
        for file in 0..8 {
            spawn_piece_at(
                &mut commands,
                &piece_meshes,
                black_material.clone(),
                PieceColor::Black,
                PieceType::Pawn,
                (6, file),
            );
        }
    }

    /// Container for piece mesh handles
    struct PieceMeshes {
        king: Handle<Mesh>,
        king_cross: Handle<Mesh>,
        pawn: Handle<Mesh>,
        knight_1: Handle<Mesh>,
        knight_2: Handle<Mesh>,
        rook: Handle<Mesh>,
        bishop: Handle<Mesh>,
        queen: Handle<Mesh>,
    }

    /// Unified piece spawning function - dispatches to specific spawner based on type
    fn spawn_piece_at(
        commands: &mut Commands,
        meshes: &PieceMeshes,
        material: Handle<StandardMaterial>,
        color: PieceColor,
        piece_type: PieceType,
        position: (u8, u8),
    ) {
        // Handle is Clone (not Copy), need .clone() from shared ref
        match piece_type {
            PieceType::King => spawn_king(commands, material, color, meshes.king.clone(), meshes.king_cross.clone(), position),
            PieceType::Queen => spawn_queen(commands, material, color, meshes.queen.clone(), position),
            PieceType::Rook => spawn_rook(commands, material, color, meshes.rook.clone(), position),
            PieceType::Bishop => spawn_bishop(commands, material, color, meshes.bishop.clone(), position),
            PieceType::Knight => spawn_knight(commands, material, color, meshes.knight_1.clone(), meshes.knight_2.clone(), position),
            PieceType::Pawn => spawn_pawn(commands, material, color, meshes.pawn.clone(), position),
        }
    }

    fn piece_transform(offset: Vec3) -> Transform {
        let mut t = Transform::from_translation(offset);
        t.scale = Vec3::splat(0.2);
        t
    }

    /// Helper function to generate piece name for inspector
    fn piece_name(piece_type: PieceType, color: PieceColor, position: (u8, u8)) -> String {
        let color_str = match color {
            PieceColor::White => "White",
            PieceColor::Black => "Black",
        };
        let piece_str = match piece_type {
            PieceType::King => "King",
            PieceType::Queen => "Queen",
            PieceType::Rook => "Rook",
            PieceType::Bishop => "Bishop",
            PieceType::Knight => "Knight",
            PieceType::Pawn => "Pawn",
        };
        let file = (b'a' + position.1) as char;
        let rank = position.0 + 1;
        format!("{} {} {}{}", color_str, piece_str, file, rank)
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
            use crate::core::GameState;

            // DespawnOnExit automatically despawns all pieces when exiting Multiplayer
            commands
                .spawn((
                    // All components in single tuple - idiomatic Bevy 0.17
                    Transform::from_translation(Vec3::new(
                        position.0 as f32,
                        0.,
                        position.1 as f32,
                    )),
                    Visibility::Inherited,
                    PointerInteraction::default(),
                    Name::new(piece_name(PieceType::King, piece_color, position)),
                    DespawnOnExit(GameState::Multiplayer),
                    Piece {
                        color: piece_color,
                        piece_type: PieceType::King,
                        x: position.0,
                        y: position.1,
                    },
                    HasMoved::default(),
                ))
                .observe(on_piece_click)   // Observer for click handling
                .observe(on_piece_hover)   // Observer for hover effect
                .observe(on_piece_unhover) // Observer for unhover effect
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
                            MeshMaterial3d(material),
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
        use crate::core::GameState;

        commands
            .spawn((
                // All components in single tuple - idiomatic Bevy 0.17
                Transform::from_translation(Vec3::new(position.0 as f32, 0., position.1 as f32)),
                Visibility::Inherited,
                PointerInteraction::default(),
                Name::new(piece_name(PieceType::Knight, piece_color, position)),
                DespawnOnExit(GameState::Multiplayer),
                Piece {
                    color: piece_color,
                    piece_type: PieceType::Knight,
                    x: position.0,
                    y: position.1,
                },
                HasMoved::default(),
            ))
            .observe(on_piece_click)   // Observer for click handling
            .observe(on_piece_hover)   // Observer for hover effect
            .observe(on_piece_unhover) // Observer for unhover effect
            .with_children(|parent| {
                parent
                    .spawn((
                        Mesh3d(mesh_1.clone()),
                        MeshMaterial3d(material.clone()),
                        piece_transform(Vec3::new(-0.2, 0., 0.9)),
                    ));
                parent
                    .spawn((
                        Mesh3d(mesh_2.clone()),
                        MeshMaterial3d(material.clone()),
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
        use crate::core::GameState;

        commands
            .spawn((
                // All components in single tuple - idiomatic Bevy 0.17
                Transform::from_translation(Vec3::new(position.0 as f32, 0., position.1 as f32)),
                Visibility::Inherited,
                PointerInteraction::default(),
                Name::new(piece_name(PieceType::Queen, piece_color, position)),
                DespawnOnExit(GameState::Multiplayer),
                Piece {
                    color: piece_color,
                    piece_type: PieceType::Queen,
                    x: position.0,
                    y: position.1,
                },
                HasMoved::default(),
            ))
            .observe(on_piece_click)   // Observer for click handling
            .observe(on_piece_hover)   // Observer for hover effect
            .observe(on_piece_unhover) // Observer for unhover effect
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
        use crate::core::GameState;

        commands
            .spawn((
                // All components in single tuple - idiomatic Bevy 0.17
                Transform::from_translation(Vec3::new(position.0 as f32, 0., position.1 as f32)),
                Visibility::Inherited,
                PointerInteraction::default(),
                Name::new(piece_name(PieceType::Bishop, piece_color, position)),
                DespawnOnExit(GameState::Multiplayer),
                Piece {
                    color: piece_color,
                    piece_type: PieceType::Bishop,
                    x: position.0,
                    y: position.1,
                },
                HasMoved::default(),
            ))
            .observe(on_piece_click)   // Observer for click handling
            .observe(on_piece_hover)   // Observer for hover effect
            .observe(on_piece_unhover) // Observer for unhover effect
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
        use crate::core::GameState;

        commands
            .spawn((
                // All components in single tuple - idiomatic Bevy 0.17
                Transform::from_translation(Vec3::new(position.0 as f32, 0., position.1 as f32)),
                Visibility::Inherited,
                PointerInteraction::default(),
                Name::new(piece_name(PieceType::Rook, piece_color, position)),
                DespawnOnExit(GameState::Multiplayer),
                Piece {
                    color: piece_color,
                    piece_type: PieceType::Rook,
                    x: position.0,
                    y: position.1,
                },
                HasMoved::default(),
            ))
            .observe(on_piece_click)   // Observer for click handling
            .observe(on_piece_hover)   // Observer for hover effect
            .observe(on_piece_unhover) // Observer for unhover effect
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
        use crate::core::GameState;

        commands
            .spawn((
                // All components in single tuple - idiomatic Bevy 0.17
                Transform::from_translation(Vec3::new(position.0 as f32, 0., position.1 as f32)),
                Visibility::Inherited,
                PointerInteraction::default(),
                Name::new(piece_name(PieceType::Pawn, piece_color, position)),
                DespawnOnExit(GameState::Multiplayer),
                Piece {
                    color: piece_color,
                    piece_type: PieceType::Pawn,
                    x: position.0,
                    y: position.1,
                },
                HasMoved::default(),
            ))
            .observe(on_piece_click)   // Observer for click handling
            .observe(on_piece_hover)   // Observer for hover effect
            .observe(on_piece_unhover) // Observer for unhover effect
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
            use crate::core::GameState;
            app.add_systems(OnEnter(GameState::Multiplayer), create_pieces);
        }
    }

