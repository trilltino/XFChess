//! Core chess piece types — the authoritative definitions for the game.
//!
//! These types represent the logical identity of chess pieces and are used
//! throughout the codebase for game logic, engine integration, and rendering.
//!
//! # Why Here (Not in `rendering`)?
//!
//! `Piece`, `PieceColor`, and `PieceType` are **game state**, not visual concerns.
//! They are queried by the engine, input system, AI, and network sync — not just
//! rendering. Placing them in `game/components` makes the dependency direction
//! correct: rendering depends on game, not the other way around.
//!
//! The `rendering::pieces` module re-exports these types for backward compatibility.
//!
//! # Reference
//!
//! - <https://doc.rust-lang.org/book/ch07-04-bringing-paths-into-scope-with-the-use-keyword.html>
//! - <https://stackoverflow.com/questions/459204> (module organisation patterns)

use bevy::prelude::*;

/// Color of a chess piece (White or Black).
#[derive(Clone, Copy, Debug, Component, PartialEq, Eq, Hash, Reflect, Default)]
#[reflect(Component)]
pub enum PieceColor {
    #[default]
    White,
    Black,
}

/// Type of a chess piece.
#[derive(
    Component,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Debug,
    Reflect,
    Default,
    serde::Serialize,
    serde::Deserialize,
)]
#[reflect(Component)]
pub enum PieceType {
    #[default]
    King,
    Queen,
    Bishop,
    Knight,
    Rook,
    Pawn,
}

impl PieceType {
    /// Parse a piece type from a single character (case-insensitive).
    ///
    /// Returns `None` for king and pawn (they are not used in promotion notation).
    pub fn from_char(c: char) -> Option<Self> {
        match c.to_ascii_lowercase() {
            'q' => Some(PieceType::Queen),
            'b' => Some(PieceType::Bishop),
            'n' => Some(PieceType::Knight),
            'r' => Some(PieceType::Rook),
            _ => None,
        }
    }
}

/// Represents a chess piece on the board.
///
/// Uses standard chess coordinates where:
/// - `x` = file (0-7, corresponds to files a-h)
/// - `y` = rank (0-7, corresponds to ranks 1-8)
///
/// # World-space mapping
///
/// World X = file (`x`), World Z = rank (`y`), World Y = height.
#[derive(Component, Clone, Debug, Copy, Reflect)]
#[reflect(Component)]
pub struct Piece {
    pub color: PieceColor,
    pub piece_type: PieceType,
    /// File (column) — 0 = a, 7 = h
    pub x: u8,
    /// Rank (row) — 0 = rank 1, 7 = rank 8
    pub y: u8,
}

impl Piece {
    /// Create a new piece with chess coordinates.
    pub fn new(color: PieceColor, piece_type: PieceType, file: u8, rank: u8) -> Self {
        Self {
            color,
            piece_type,
            x: file,
            y: rank,
        }
    }

    /// Get the file (0-7, a-h).
    #[inline]
    pub fn file(&self) -> u8 {
        self.x
    }

    /// Get the rank (0-7, 1-8).
    #[inline]
    pub fn rank(&self) -> u8 {
        self.y
    }

    /// Convert to world position (XZ plane).
    ///
    /// Returns `(world_x, world_z)` where `world_x = file`, `world_z = rank`.
    pub fn to_world(&self) -> (f32, f32) {
        (self.x as f32, self.y as f32)
    }
}
