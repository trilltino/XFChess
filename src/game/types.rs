//! Type definitions and utilities for chess game logic
//!
//! Provides newtype patterns and trait implementations for chess-specific types
//! to improve type safety and code clarity.

use crate::rendering::pieces::PieceType;

/// Board coordinate representing a file (column) on the chessboard
///
/// Values range from 0 (file 'a') to 7 (file 'h').
/// This newtype prevents mixing up x and y coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct File(pub u8);

impl File {
    /// Create a file from a character ('a'..='h')
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let file = File::from_char('e').unwrap(); // File 4
    /// ```
    pub fn from_char(c: char) -> Option<Self> {
        match c {
            'a'..='h' => Some(File((c as u8 - b'a') as u8)),
            _ => None,
        }
    }

    /// Convert file to character ('a'..='h')
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let file = File(4);
    /// assert_eq!(file.to_char(), 'e');
    /// ```
    pub fn to_char(self) -> char {
        (b'a' + self.0) as char
    }

    /// Get the file index (0-7)
    pub fn index(self) -> u8 {
        self.0
    }
}

impl From<u8> for File {
    fn from(value: u8) -> Self {
        assert!(value < 8, "File must be in range 0-7");
        File(value)
    }
}

impl From<File> for u8 {
    fn from(file: File) -> Self {
        file.0
    }
}

/// Board coordinate representing a rank (row) on the chessboard
///
/// Values range from 0 (rank 1) to 7 (rank 8).
/// This newtype prevents mixing up x and y coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Rank(pub u8);

impl Rank {
    /// Create a rank from a number (1-8)
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let rank = Rank::from_number(4).unwrap(); // Rank 3 (0-indexed)
    /// ```
    pub fn from_number(n: u8) -> Option<Self> {
        if (1..=8).contains(&n) {
            Some(Rank(n - 1))
        } else {
            None
        }
    }

    /// Convert rank to number (1-8)
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let rank = Rank(3);
    /// assert_eq!(rank.to_number(), 4);
    /// ```
    pub fn to_number(self) -> u8 {
        self.0 + 1
    }

    /// Get the rank index (0-7)
    pub fn index(self) -> u8 {
        self.0
    }
}

impl From<u8> for Rank {
    fn from(value: u8) -> Self {
        assert!(value < 8, "Rank must be in range 0-7");
        Rank(value)
    }
}

impl From<Rank> for u8 {
    fn from(rank: Rank) -> Self {
        rank.0
    }
}

/// Board square position (file, rank)
///
/// Combines File and Rank into a single type-safe coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Square {
    pub file: File,
    pub rank: Rank,
}

impl Square {
    /// Create a square from file and rank indices
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let square = Square::new(4, 3); // e4
    /// ```
    pub fn new(file: u8, rank: u8) -> Self {
        Square {
            file: File::from(file),
            rank: Rank::from(rank),
        }
    }

    /// Create a square from algebraic notation (e.g., "e4")
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let square = Square::from_algebraic("e4").unwrap();
    /// ```
    pub fn from_algebraic(s: &str) -> Option<Self> {
        let mut chars = s.chars();
        let file_char = chars.next()?;
        let rank_char = chars.next()?;
        let rank_num = rank_char.to_digit(10)? as u8;

        Some(Square {
            file: File::from_char(file_char)?,
            rank: Rank::from_number(rank_num)?,
        })
    }

    /// Convert square to algebraic notation (e.g., "e4")
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let square = Square::new(4, 3);
    /// assert_eq!(square.to_algebraic(), "e4");
    /// ```
    pub fn to_algebraic(self) -> String {
        format!("{}{}", self.file.to_char(), self.rank.to_number())
    }

    /// Convert to tuple (x, y) for compatibility with existing code
    pub fn to_tuple(self) -> (u8, u8) {
        (self.file.index(), self.rank.index())
    }

    /// Create from tuple (x, y) for compatibility with existing code
    pub fn from_tuple((x, y): (u8, u8)) -> Self {
        Square::new(x, y)
    }
}

impl From<(u8, u8)> for Square {
    fn from((x, y): (u8, u8)) -> Self {
        Square::from_tuple((x, y))
    }
}

impl From<Square> for (u8, u8) {
    fn from(square: Square) -> Self {
        square.to_tuple()
    }
}

/// Piece value in centipawns for material evaluation
///
/// Standard chess piece values:
/// - Pawn: 100 centipawns
/// - Knight/Bishop: 300 centipawns
/// - Rook: 500 centipawns
/// - Queen: 900 centipawns
/// - King: 0 (cannot be captured)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Centipawns(pub i32);

impl Centipawns {
    /// Get the standard value for a piece type
    pub fn for_piece(piece_type: PieceType) -> Self {
        match piece_type {
            PieceType::Pawn => Centipawns(100),
            PieceType::Knight => Centipawns(300),
            PieceType::Bishop => Centipawns(300),
            PieceType::Rook => Centipawns(500),
            PieceType::Queen => Centipawns(900),
            PieceType::King => Centipawns(0), // King cannot be captured
        }
    }

    /// Convert to pawns (divide by 100)
    pub fn to_pawns(self) -> f32 {
        self.0 as f32 / 100.0
    }

    /// Get the raw centipawn value
    pub fn value(self) -> i32 {
        self.0
    }
}

impl From<i32> for Centipawns {
    fn from(value: i32) -> Self {
        Centipawns(value)
    }
}

impl From<Centipawns> for i32 {
    fn from(cp: Centipawns) -> Self {
        cp.0
    }
}

impl std::ops::Add for Centipawns {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Centipawns(self.0 + other.0)
    }
}

impl std::ops::Sub for Centipawns {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Centipawns(self.0 - other.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_to_char() {
        assert_eq!(File(0).to_char(), 'a');
        assert_eq!(File(4).to_char(), 'e');
        assert_eq!(File(7).to_char(), 'h');
    }

    #[test]
    fn test_rank_from_number() {
        assert_eq!(Rank::from_number(1), Some(Rank(0)));
        assert_eq!(Rank::from_number(4), Some(Rank(3)));
        assert_eq!(Rank::from_number(8), Some(Rank(7)));
        assert_eq!(Rank::from_number(0), None);
        assert_eq!(Rank::from_number(9), None);
    }

    #[test]
    fn test_rank_to_number() {
        assert_eq!(Rank(0).to_number(), 1);
        assert_eq!(Rank(3).to_number(), 4);
        assert_eq!(Rank(7).to_number(), 8);
    }

    #[test]
    fn test_square_algebraic() {
        let square = Square::from_algebraic("e4").unwrap();
        assert_eq!(square.file.index(), 4);
        assert_eq!(square.rank.index(), 3);
        assert_eq!(square.to_algebraic(), "e4");

        let square2 = Square::from_algebraic("a1").unwrap();
        assert_eq!(square2.file.index(), 0);
        assert_eq!(square2.rank.index(), 0);
    }

    #[test]
    fn test_centipawns() {
        assert_eq!(Centipawns::for_piece(PieceType::Pawn).value(), 100);
        assert_eq!(Centipawns::for_piece(PieceType::Queen).value(), 900);
        assert_eq!(Centipawns::for_piece(PieceType::King).value(), 0);

        let cp1 = Centipawns(300);
        let cp2 = Centipawns(500);
        assert_eq!((cp1 + cp2).value(), 800);
        assert_eq!((cp2 - cp1).value(), 200);
    }
}
