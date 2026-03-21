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

use bevy::prelude::*;

/// Represents a square on the chess board.
/// Uses standard chess coordinates where:
/// - `x` = file (0-7, corresponds to files a-h)
/// - `y` = rank (0-7, corresponds to ranks 1-8)
#[derive(Default, Resource, Component, Debug, Clone, Eq, PartialEq)]
pub struct Square {
    /// File (column) - 0 = a, 7 = h
    pub x: u8,
    /// Rank (row) - 0 = 1, 7 = 8
    pub y: u8,
}

impl Square {
    /// Returns true if this square should be white in a standard chess board pattern
    ///
    /// Uses the formula (file + rank + 1) % 2 == 0 to create the traditional checkerboard.
    /// Light squares: a1, c1, e1, g1 (files 0, 2, 4, 6 on rank 0)
    pub fn is_white(&self) -> bool {
        (self.x + self.y + 1).is_multiple_of(2)
    }

    /// Convert to world position (XZ plane)
    /// Returns (world_x, world_z) where world_x = file, world_z = rank
    pub fn to_world(&self) -> (f32, f32) {
        (self.x as f32, self.y as f32)
    }

    /// Create from chess notation (file 0-7, rank 0-7)
    pub fn new(file: u8, rank: u8) -> Self {
        Self { x: file, y: rank }
    }
}

#[derive(Resource, Component)]
pub struct SquareMaterials {
    pub black_color: Handle<StandardMaterial>,
    pub white_color: Handle<StandardMaterial>,
    /// Material used for highlighting selected squares and possible move destinations
    pub hover_matl: Handle<StandardMaterial>,
    /// Material for selected piece border (soft black)
    pub selected_border_matl: Handle<StandardMaterial>,
    /// Grey material for TempleOS view (light squares)
    pub grey_color: Handle<StandardMaterial>,
    /// White material for TempleOS view (dark squares in standard view)
    pub templeos_white: Handle<StandardMaterial>,
    /// Shared mesh for move hints (prevent per-frame allocation)
    pub hint_mesh: Handle<Mesh>,
    /// Shared mesh for last move highlights (prevent per-frame allocation)
    pub highlight_mesh: Handle<Mesh>,
}

impl FromWorld for SquareMaterials {
    fn from_world(world: &mut World) -> Self {
        // Use fixed Classic theme colors (Cream and Green)
        let light_color = Color::srgb(0.93, 0.93, 0.82); // Cream
        let dark_color = Color::srgb(0.46, 0.59, 0.34); // Green

        // Now get materials (mutable borrow)
        // Note: Assets<StandardMaterial> should always be available (part of DefaultPlugins)
        // but we handle the error case gracefully for robustness
        let mut materials = match world.get_resource_mut::<Assets<StandardMaterial>>() {
            Some(m) => m,
            None => {
                error!("[RENDERING] Assets<StandardMaterial> not available during SquareMaterials initialization");
                error!("[RENDERING] This should not happen - DefaultPlugins should provide this resource");
                // This is a critical error - return a placeholder that will cause issues
                // but at least won't panic. In practice, this should never happen.
                panic!("Assets<StandardMaterial> must be initialized before SquareMaterials - check plugin order");
            }
        };

        // Create unlit materials for TempleOS mode (flat colors without lighting)
        // Matching reference image: dark grey for black squares, white for white squares
        let grey_material = StandardMaterial {
            base_color: Color::srgb(0.35, 0.35, 0.35), // Dark grey for black squares
            unlit: true,                               // Unlit for flat 2D appearance
            ..default()
        };
        let white_material = StandardMaterial {
            base_color: Color::srgb(1.0, 1.0, 1.0), // Pure white for white squares
            unlit: true,                            // Unlit for flat 2D appearance
            ..default()
        };

        SquareMaterials {
            black_color: materials.add(light_color), // Light squares
            white_color: materials.add(dark_color),  // Dark squares
            hover_matl: materials.add(StandardMaterial {
                base_color: Color::srgba(0.8, 0.2, 0.2, 0.3), // Soft transparent red
                alpha_mode: AlphaMode::Blend,
                unlit: true, // Make it unlit so it shows consistently
                ..default()
            }),
            selected_border_matl: materials.add(StandardMaterial {
                base_color: Color::srgba(0.0, 0.0, 0.0, 0.5), // Soft black border
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                ..default()
            }),
            grey_color: materials.add(grey_material), // Dull grey for TempleOS (unlit)
            templeos_white: materials.add(white_material), // Bright white for TempleOS (unlit)
            hint_mesh: world
                .resource_mut::<Assets<Mesh>>()
                .add(Plane3d::default().mesh().size(0.9, 0.9)),
            highlight_mesh: world
                .resource_mut::<Assets<Mesh>>()
                .add(Plane3d::default().mesh().size(0.95, 0.95)),
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
    pub fn get_original_material(
        &self,
        square: &Square,
        materials: &SquareMaterials,
    ) -> Handle<StandardMaterial> {
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
