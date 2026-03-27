//! Typed board coordinate to replace raw `(u8, u8)` tuples.
//!
//! `BoardPos` encodes a chess square as `(file, rank)` where:
//! - `file` = 0-7 (a-h)
//! - `rank` = 0-7 (1-8)
//!
//! Using a named struct instead of a bare tuple prevents the common
//! mistake of swapping file and rank.
//!
//! # World-space mapping
//!
//! Bevy world coordinates: X = file, Z = rank (Y is up).
//!
//! # Reference
//!
//! - <https://en.wikipedia.org/wiki/Algebraic_notation_(chess)>
//! - <https://stackoverflow.com/questions/16523> (SQL-style indexing pitfalls)

use bevy::prelude::*;

/// A typed chess board position.
///
/// Prevents the file/rank swap bugs that plague raw `(u8, u8)` tuples.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, Reflect)]
pub struct BoardPos {
    /// File (column): 0 = a, 7 = h
    pub file: u8,
    /// Rank (row): 0 = rank 1, 7 = rank 8
    pub rank: u8,
}

impl BoardPos {
    /// Create a new board position.
    ///
    /// # Panics (debug only)
    ///
    /// Debug-asserts that file and rank are in 0..8.
    #[inline]
    pub const fn new(file: u8, rank: u8) -> Self {
        debug_assert!(file < 8, "file must be 0-7");
        debug_assert!(rank < 8, "rank must be 0-7");
        Self { file, rank }
    }

    /// Convert to a `(file, rank)` tuple for interop with legacy code.
    #[inline]
    pub const fn as_tuple(self) -> (u8, u8) {
        (self.file, self.rank)
    }

    /// Build from a `(file, rank)` tuple.
    #[inline]
    pub const fn from_tuple(t: (u8, u8)) -> Self {
        Self::new(t.0, t.1)
    }

    /// File character ('a'–'h').
    #[inline]
    pub const fn file_char(self) -> char {
        (b'a' + self.file) as char
    }

    /// Rank number (1–8) as displayed on a chess board.
    #[inline]
    pub const fn rank_display(self) -> u8 {
        self.rank + 1
    }

    /// UCI square string, e.g. `"e4"`.
    pub fn to_uci(self) -> String {
        format!("{}{}", self.file_char(), self.rank_display())
    }

    /// Parse a UCI square string (e.g. `"e4"`) into a `BoardPos`.
    pub fn from_uci(s: &str) -> Option<Self> {
        let bytes = s.as_bytes();
        if bytes.len() < 2 {
            return None;
        }
        let file = bytes[0].wrapping_sub(b'a');
        let rank = bytes[1].wrapping_sub(b'1');
        if file < 8 && rank < 8 {
            Some(Self { file, rank })
        } else {
            None
        }
    }

    /// Flat index in a 64-element array (rank-major: `rank * 8 + file`).
    #[inline]
    pub const fn index(self) -> usize {
        (self.rank as usize) * 8 + self.file as usize
    }

    /// Reconstruct from a flat index.
    #[inline]
    pub const fn from_index(idx: usize) -> Self {
        Self {
            file: (idx % 8) as u8,
            rank: (idx / 8) as u8,
        }
    }

    /// World-space X coordinate (file maps to X).
    #[inline]
    pub fn world_x(self) -> f32 {
        self.file as f32
    }

    /// World-space Z coordinate (rank maps to Z).
    #[inline]
    pub fn world_z(self) -> f32 {
        self.rank as f32
    }
}

impl std::fmt::Display for BoardPos {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", self.file_char(), self.rank_display())
    }
}

impl From<(u8, u8)> for BoardPos {
    /// Convert `(file, rank)` tuple to `BoardPos`.
    fn from(t: (u8, u8)) -> Self {
        Self::new(t.0, t.1)
    }
}

impl From<BoardPos> for (u8, u8) {
    fn from(pos: BoardPos) -> Self {
        pos.as_tuple()
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_and_accessors() {
        let pos = BoardPos::new(4, 3); // e4
        assert_eq!(pos.file, 4);
        assert_eq!(pos.rank, 3);
        assert_eq!(pos.file_char(), 'e');
        assert_eq!(pos.rank_display(), 4);
    }

    #[test]
    fn uci_roundtrip() {
        for file in 0..8u8 {
            for rank in 0..8u8 {
                let pos = BoardPos::new(file, rank);
                let uci = pos.to_uci();
                let parsed = BoardPos::from_uci(&uci).expect("valid UCI");
                assert_eq!(pos, parsed, "roundtrip failed for {uci}");
            }
        }
    }

    #[test]
    fn index_roundtrip() {
        for i in 0..64 {
            let pos = BoardPos::from_index(i);
            assert_eq!(pos.index(), i);
        }
    }

    #[test]
    fn tuple_conversion() {
        let pos = BoardPos::new(2, 5);
        let t: (u8, u8) = pos.into();
        assert_eq!(t, (2, 5));
        let back: BoardPos = t.into();
        assert_eq!(back, pos);
    }

    #[test]
    fn display_format() {
        assert_eq!(format!("{}", BoardPos::new(0, 0)), "a1");
        assert_eq!(format!("{}", BoardPos::new(4, 3)), "e4");
        assert_eq!(format!("{}", BoardPos::new(7, 7)), "h8");
    }

    #[test]
    fn from_uci_edge_cases() {
        assert_eq!(BoardPos::from_uci(""), None);
        assert_eq!(BoardPos::from_uci("z9"), None);
        assert_eq!(BoardPos::from_uci("a1"), Some(BoardPos::new(0, 0)));
        assert_eq!(BoardPos::from_uci("h8"), Some(BoardPos::new(7, 7)));
    }
}
