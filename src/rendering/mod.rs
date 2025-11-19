//! Rendering module - 3D chess visualization with Bevy 0.17
//!
//! Manages all visual aspects of the chess game using Bevy's 3D rendering pipeline,
//! including mesh spawning, material management, and scene setup.
//!
//! # Architecture
//!
//! - `board` - Chess board mesh and square generation
//! - `pieces` - 3D piece models loaded from GLTF with material variants
//! - `utils` - Rendering utilities (materials, square component)
//!
//! # Bevy 0.17 Rendering
//!
//! Uses modern Bevy 0.17 rendering components:
//! - `Mesh3d` - Mesh handle component (was `Handle<Mesh>` in 0.16)
//! - `MeshMaterial3d<StandardMaterial>` - Material component
//! - `Transform` - Position, rotation, scale
//! - `PointerInteraction` - Built-in picking support
//!
//! # Reference Materials
//!
//! - `reference/bevy/examples/3d/` - 3D rendering patterns
//! - `reference/bevy-3d-chess/` - Alternative chess rendering approach
//! - `assets/models/chess_kit/pieces.glb` - GLTF model source
//!
//! The GLTF model is structured with separate meshes for each piece type, allowing
//! selective loading via asset path fragments (e.g., `#Mesh0/Primitive0` for king).
//!
//! # Performance Considerations
//!
//! - Piece models are loaded once and cloned via `Handle<Mesh>`
//! - Materials are shared between same-colored pieces
//! - Board uses instanced rendering for 64 square meshes

// Submodules
pub mod board;
pub mod camera;
pub mod effects;
pub mod pieces;

// Root-level modules
pub mod graphics_quality;
pub mod utils;

// Re-export commonly used items
pub use board::*;
pub use camera::*;
pub use effects::*;
pub use pieces::*;
pub use utils::*;
