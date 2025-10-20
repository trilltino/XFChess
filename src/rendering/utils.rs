//! Board rendering utilities - Square materials and coloring
//!
//! This module provides resources and utilities for managing the visual appearance
//! of the chess board squares, including:
//!
//! - **Square**: Component identifying a board square's position
//! - **SquareMaterials**: Resource holding material handles for square colors
//! - **ReturnMaterials**: Utility for restoring original square colors after hover effects
//!
//! # Chess Board Coloring
//!
//! Traditional chess boards alternate between light and dark squares. The `is_white()`
//! method uses the formula `(x + y + 1) % 2 == 0` to determine square color based on
//! grid position, creating the standard checkerboard pattern.
//!
//! # Future Features
//!
//! Reserved functionality includes:
//! - Hover effects when cursor moves over squares
//! - Highlight effects for selected pieces
//! - Attack visualization for check/checkmate
//!
//! # Bevy 0.17 Integration
//!
//! Uses `FromWorld` trait for resource initialization with access to `World` for
//! creating materials. This pattern is standard for resources that need asset handles
//! during initialization.
//!
//! # Reference
//!
//! Material management follows patterns from:
//! - `reference/bevy/examples/3d/3d_shapes.rs` - Material creation and usage
//! - `reference/bevy/examples/asset/hot_asset_reloading.rs` - Handle management

    use bevy::color::palettes::tailwind::*;
    use bevy::prelude::*;

    #[derive(Default, Resource, Component, Debug, Clone, Eq, PartialEq)]
    pub struct Square {
            pub x: u8,
            pub y: u8,
        }

    impl Square {
        /// Returns true if this square should be white in a standard chess board pattern
        ///
        /// Uses the formula (x + y + 1) % 2 == 0 to create the traditional checkerboard.
        /// This method is used during board creation to assign correct square colors.
        pub fn is_white(&self) -> bool {
            (self.x + self.y + 1).is_multiple_of(2)
        }
    }

        
    #[derive(Resource, Component)]
    pub struct SquareMaterials {
            pub black_color: Handle<StandardMaterial>,
            pub white_color: Handle<StandardMaterial>,
            /// Material used for highlighting selected squares and possible move destinations
            pub hover_matl:  Handle<StandardMaterial>,
        }

    impl FromWorld for SquareMaterials {
        fn from_world(world: &mut World) -> Self {
            let mut materials = world.get_resource_mut::<Assets<StandardMaterial>>()
            .expect("Assets<StandardMaterial> should be initialized before SquareMaterials");
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
        /// Returns the original material for a square based on its color
        ///
        /// This method is used to restore square colors after hover/selection effects.
        /// White squares get black_color material, black squares get white_color material.
        /// Handle is Clone (not Copy), so we clone from Res
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_square_default() {
        //! Verifies Square defaults to (0, 0) position
        let square = Square::default();
        assert_eq!(square.x, 0);
        assert_eq!(square.y, 0);
    }

    #[test]
    fn test_square_clone() {
        //! Tests Square can be cloned correctly
        let square = Square { x: 3, y: 5 };
        let cloned = square.clone();
        assert_eq!(square, cloned);
    }

    #[test]
    fn test_square_equality() {
        //! Verifies Square equality comparisons work
        let sq1 = Square { x: 2, y: 4 };
        let sq2 = Square { x: 2, y: 4 };
        let sq3 = Square { x: 1, y: 4 };

        assert_eq!(sq1, sq2);
        assert_ne!(sq1, sq3);
    }

    #[test]
    fn test_square_is_white_a1() {
        //! Tests square a1 (0,0) is black in our chess board coloring
        let square = Square { x: 0, y: 0 };
        assert!(!square.is_white());
    }

    #[test]
    fn test_square_is_white_h1() {
        //! Tests square h1 (0,7) is white in our chess board coloring
        let square = Square { x: 0, y: 7 };
        assert!(square.is_white());
    }

    #[test]
    fn test_square_is_white_a8() {
        //! Tests square a8 (7,0) is white in our chess board coloring
        let square = Square { x: 7, y: 0 };
        assert!(square.is_white());
    }

    #[test]
    fn test_square_is_white_h8() {
        //! Tests square h8 (7,7) is black in our chess board coloring
        let square = Square { x: 7, y: 7 };
        assert!(!square.is_white());
    }

    #[test]
    fn test_square_checkerboard_pattern() {
        //! Verifies alternating checkerboard pattern across the board
        //!
        //! Adjacent squares (horizontally or vertically) should always
        //! have opposite colors.
        for x in 0..7 {
            for y in 0..8 {
                let sq1 = Square { x, y };
                let sq2 = Square { x: x + 1, y };

                // Horizontal neighbors should differ
                assert_ne!(sq1.is_white(), sq2.is_white());
            }
        }

        for x in 0..8 {
            for y in 0..7 {
                let sq1 = Square { x, y };
                let sq2 = Square { x, y: y + 1 };

                // Vertical neighbors should differ
                assert_ne!(sq1.is_white(), sq2.is_white());
            }
        }
    }

    #[test]
    fn test_square_diagonal_same_color() {
        //! Verifies diagonal squares share the same color
        //!
        //! In chess, all squares on a diagonal have the same color.
        let sq1 = Square { x: 0, y: 0 };
        let sq2 = Square { x: 2, y: 2 };
        let sq3 = Square { x: 4, y: 4 };

        assert_eq!(sq1.is_white(), sq2.is_white());
        assert_eq!(sq2.is_white(), sq3.is_white());
    }

    #[test]
    fn test_return_materials_default() {
        //! Tests ReturnMaterials can be created with default
        let _materials = ReturnMaterials::default();
        // If this compiles and runs, Default works correctly
    }
}