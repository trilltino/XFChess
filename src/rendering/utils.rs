use bevy::prelude::*;

#[derive(Default, Component, Debug, Clone, Eq, PartialEq)]
pub struct Square {
    pub x: u8,
    pub y: u8,
}

impl Square {
    pub fn is_white(&self) -> bool {
        (self.x + self.y + 1).is_multiple_of(2)
    }

    pub fn new(file: u8, rank: u8) -> Self {
        Self { x: file, y: rank }
    }
}

#[derive(Resource)]
pub struct SquareMaterials {
    pub black_color: Handle<StandardMaterial>,
    pub white_color: Handle<StandardMaterial>,
    pub hover_matl: Handle<StandardMaterial>,
    pub selected_border_matl: Handle<StandardMaterial>,
    pub hint_mesh: Handle<Mesh>,
    pub capture_hint_mesh: Handle<Mesh>,
    pub capture_hint_matl: Handle<StandardMaterial>,
    pub highlight_mesh: Handle<Mesh>,
}

impl FromWorld for SquareMaterials {
    fn from_world(world: &mut World) -> Self {
        // Use fixed Classic theme colors (Cream and Green)
        let light_color = Color::srgb(0.97, 0.97, 0.88); // Cream
        let dark_color = Color::srgb(0.52, 0.65, 0.40); // Green

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

        SquareMaterials {
            black_color: materials.add(light_color), // Light squares
            white_color: materials.add(dark_color),  // Dark squares
            hover_matl: materials.add(StandardMaterial {
                base_color: Color::srgba(0.18, 0.78, 0.35, 0.82), // Vivid green (Lichess-style move dots)
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                ..default()
            }),
            selected_border_matl: materials.add(StandardMaterial {
                base_color: Color::srgba(0.95, 0.85, 0.1, 0.75), // Bright gold for selected square
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                ..default()
            }),
            capture_hint_matl: materials.add(StandardMaterial {
                base_color: Color::srgba(0.90, 0.25, 0.08, 0.85),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                ..default()
            }),
            hint_mesh: world.resource_mut::<Assets<Mesh>>().add(Circle::new(0.28)),
            capture_hint_mesh: world
                .resource_mut::<Assets<Mesh>>()
                .add(Annulus::new(0.38, 0.48)),
            highlight_mesh: world
                .resource_mut::<Assets<Mesh>>()
                .add(Plane3d::default().mesh().size(0.92, 0.92)),
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
        let square = Square::default();
        assert_eq!(square.x, 0);
        assert_eq!(square.y, 0);
    }

    #[test]
    fn test_square_clone() {
        let square = Square { x: 3, y: 5 };
        let cloned = square.clone();
        assert_eq!(square, cloned);
    }

    #[test]
    fn test_square_equality() {
        let sq1 = Square { x: 2, y: 4 };
        let sq2 = Square { x: 2, y: 4 };
        let sq3 = Square { x: 1, y: 4 };

        assert_eq!(sq1, sq2);
        assert_ne!(sq1, sq3);
    }

    #[test]
    fn test_square_is_white_a1() {
        let square = Square { x: 0, y: 0 };
        assert!(!square.is_white());
    }

    #[test]
    fn test_square_is_white_h1() {
        let square = Square { x: 0, y: 7 };
        assert!(square.is_white());
    }

    #[test]
    fn test_square_is_white_a8() {
        let square = Square { x: 7, y: 0 };
        assert!(square.is_white());
    }

    #[test]
    fn test_square_is_white_h8() {
        let square = Square { x: 7, y: 7 };
        assert!(!square.is_white());
    }

    #[test]
    fn test_square_checkerboard_pattern() {
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
        let sq1 = Square { x: 0, y: 0 };
        let sq2 = Square { x: 2, y: 2 };
        let sq3 = Square { x: 4, y: 4 };

        assert_eq!(sq1.is_white(), sq2.is_white());
        assert_eq!(sq2.is_white(), sq3.is_white());
    }

    #[test]
    fn test_return_materials_default() {
        let _materials = ReturnMaterials::default();
        // If this compiles and runs, Default works correctly
    }
}
