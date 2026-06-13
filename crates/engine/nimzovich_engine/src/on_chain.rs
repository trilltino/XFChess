//! On-chain game state — zero allocation, no std, no search tables.
//!
//! `OnChainGame` is the minimal representation required for move validation
//! inside a Solana smart contract. It has:
//! - No `Vec`, no `Box`, no `Arc`, no `Mutex`
//! - No transposition table (80 MB on the heap — impossible on-chain)
//! - No pre-computed move-table arrays (384 Vec allocations in `new_game`)
//! - Bitboards stored as plain `u64` for O(1) check detection
//!
//! The 68-byte `CompactBoard` blob is the on-chain serialized form.
//! It is produced client-side from a FEN string and verified on-chain
//! via a zero-parse bytemuck cast.

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
#[cfg(not(feature = "std"))]
use alloc::string::String;
#[cfg(not(feature = "std"))]
use alloc::boxed::Box;
#[cfg(not(feature = "std"))]
use alloc::sync::Arc;


#[cfg(not(feature = "std"))]
use core::str;

use crate::constants::*;

// ---------------------------------------------------------------------------
// Castling flag bits stored in CompactBoard.castling
// ---------------------------------------------------------------------------
pub const CASTLE_WK: u8 = 0b0001; // White kingside  (K)
pub const CASTLE_WQ: u8 = 0b0010; // White queenside (Q)
pub const CASTLE_BK: u8 = 0b0100; // Black kingside  (k)
pub const CASTLE_BQ: u8 = 0b1000; // Black queenside (q)

// ---------------------------------------------------------------------------
// CompactBoard — 68-byte serialised board state
// ---------------------------------------------------------------------------

/// 68-byte board blob stored in the on-chain `Game` account.
/// Layout is `repr(C)` so bytemuck can cast it with zero cost.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct CompactBoard {
    /// Signed piece IDs: +1..+6 = white P/N/B/R/Q/K, -1..-6 = black, 0 = empty.
    pub squares: [i8; 64],
    /// Castling availability bits: CASTLE_WK | CASTLE_WQ | CASTLE_BK | CASTLE_BQ.
    pub castling: u8,
    /// En-passant target square (0-63), or -1 if none.
    pub ep_target: i8,
    /// Side to move: 1 = white, -1 = black.
    pub side_to_move: i8,
    pub _pad: u8,
}

impl Default for CompactBoard {
    fn default() -> Self {
        Self {
            squares: SETUP,
            castling: CASTLE_WK | CASTLE_WQ | CASTLE_BK | CASTLE_BQ,
            ep_target: -1,
            side_to_move: 1,
            _pad: 0,
        }
    }
}

impl CompactBoard {
    /// Starting position.
    pub fn starting_position() -> Self {
        Self::default()
    }

    /// Parse from raw 68-byte slice (zero-copy on-chain path).
    pub fn from_bytes(b: &[u8; 68]) -> Self {
        // SAFETY: CompactBoard is repr(C), all bit patterns are valid for i8/u8.
        unsafe { core::mem::transmute(*b) }
    }

    /// Serialise to raw 68 bytes (for storing back into the account).
    pub fn to_bytes(self) -> [u8; 68] {
        unsafe { core::mem::transmute(self) }
    }

    /// Build from a FEN string.
    pub fn from_fen(fen: &str) -> Self {
        let mut cb = Self {
            squares: [0i8; 64],
            castling: 0,
            ep_target: -1,
            side_to_move: 1,
            _pad: 0,
        };

        let mut parts = fen.split_whitespace();

        // 1. Piece placement
        if let Some(placement) = parts.next() {
            let mut sq: usize = 56; // start at a8 (top-left in FEN)
            for ch in placement.chars() {
                match ch {
                    '/' => {
                        // Move down one rank
                        if sq >= 8 { sq -= 16; } // already advanced by 8; go back 16
                    }
                    '1'..='8' => {
                        sq += ch as usize - '0' as usize;
                    }
                    c => {
                        // Guard against malformed FEN (over-full rank) running `sq`
                        // past the board and panicking on the index write.
                        if sq < 64 {
                            cb.squares[sq] = match c {
                                'P' =>  1, 'N' =>  2, 'B' =>  3, 'R' =>  4, 'Q' =>  5, 'K' =>  6,
                                'p' => -1, 'n' => -2, 'b' => -3, 'r' => -4, 'q' => -5, 'k' => -6,
                                _ => 0,
                            };
                        }
                        sq += 1;
                    }
                }
            }
        }

        // 2. Side to move
        if let Some(stm) = parts.next() {
            cb.side_to_move = if stm == "b" { -1 } else { 1 };
        }

        // 3. Castling
        if let Some(cast) = parts.next() {
            if cast.contains('K') { cb.castling |= CASTLE_WK; }
            if cast.contains('Q') { cb.castling |= CASTLE_WQ; }
            if cast.contains('k') { cb.castling |= CASTLE_BK; }
            if cast.contains('q') { cb.castling |= CASTLE_BQ; }
        }

        // 4. En-passant target square
        if let Some(ep) = parts.next() {
            if ep != "-" && ep.len() == 2 {
                let file = ep.as_bytes()[0].wrapping_sub(b'a') as i8;
                let rank = ep.as_bytes()[1].wrapping_sub(b'1') as i8;
                if (0..8).contains(&file) && (0..8).contains(&rank) {
                    cb.ep_target = rank * 8 + file;
                }
            }
        }

        cb
    }

    /// Convert to the `OnChainGame` struct used for validation logic.
    pub fn to_on_chain_game(self) -> OnChainGame {
        let mut g = OnChainGame {
            board: self.squares,
            white_pawns:   0,
            white_knights: 0,
            white_bishops: 0,
            white_rooks:   0,
            white_queens:  0,
            white_kings:   0,
            black_pawns:   0,
            black_knights: 0,
            black_bishops: 0,
            black_rooks:   0,
            black_queens:  0,
            black_kings:   0,
            occupied_white: 0,
            occupied_black: 0,
            occupied:      0,
            castling:      self.castling,
            ep_target:     self.ep_target,
            side_to_move:  self.side_to_move,
        };
        g.rebuild_bitboards();
        g
    }

    /// Convert to a FEN string.
    pub fn to_fen(&self) -> String {
        let mut fen = String::with_capacity(100);

        // 1. Piece placement
        for rank in (0..8).rev() {
            let mut empty_count = 0;
            for file in 0..8 {
                let sq = rank * 8 + file;
                let piece = self.squares[sq];
                if piece == 0 {
                    empty_count += 1;
                } else {
                    if empty_count > 0 {
                        fen.push(char::from_digit(empty_count as u32, 10).unwrap_or('1'));
                        empty_count = 0;
                    }
                    fen.push(match piece {
                         1 => 'P',  2 => 'N',  3 => 'B',  4 => 'R',  5 => 'Q',  6 => 'K',
                        -1 => 'p', -2 => 'n', -3 => 'b', -4 => 'r', -5 => 'q', -6 => 'k',
                        _ => '?',
                    });
                }
            }
            if empty_count > 0 {
                fen.push(char::from_digit(empty_count as u32, 10).unwrap_or('1'));
            }
            if rank > 0 { fen.push('/'); }
        }

        // 2. Side to move
        fen.push(' ');
        fen.push(if self.side_to_move > 0 { 'w' } else { 'b' });

        // 3. Castling
        fen.push(' ');
        if self.castling == 0 {
            fen.push('-');
        } else {
            if self.castling & CASTLE_WK != 0 { fen.push('K'); }
            if self.castling & CASTLE_WQ != 0 { fen.push('Q'); }
            if self.castling & CASTLE_BK != 0 { fen.push('k'); }
            if self.castling & CASTLE_BQ != 0 { fen.push('q'); }
        }

        // 4. En passant
        fen.push(' ');
        if self.ep_target < 0 || self.ep_target > 63 {
            fen.push('-');
        } else {
            let file = (self.ep_target % 8) as u8;
            let rank = (self.ep_target / 8) as u8;
            fen.push((b'a' + file) as char);
            fen.push((b'1' + rank) as char);
        }

        // 5. Halfmove clock & fullmove number (not tracked in CompactBoard)
        fen.push_str(" 0 1");

        fen
    }
}

// ---------------------------------------------------------------------------
// OnChainGame — zero-alloc validation state
// ---------------------------------------------------------------------------

/// Compact, zero-allocation game state for on-chain move validation.
///
/// All fields are plain integers — no `Vec`, no `Box`, no `Arc`.
/// Bitboards are `u64` rather than the engine's `BitSet` wrapper.
#[derive(Copy, Clone, Debug)]
pub struct OnChainGame {
    pub board: [i8; 64],

    // Bitboards (plain u64)
    pub white_pawns:   u64,
    pub white_knights: u64,
    pub white_bishops: u64,
    pub white_rooks:   u64,
    pub white_queens:  u64,
    pub white_kings:   u64,
    pub black_pawns:   u64,
    pub black_knights: u64,
    pub black_bishops: u64,
    pub black_rooks:   u64,
    pub black_queens:  u64,
    pub black_kings:   u64,
    pub occupied_white: u64,
    pub occupied_black: u64,
    pub occupied:      u64,

    // Game state
    pub castling:     u8,  // CASTLE_WK | CASTLE_WQ | CASTLE_BK | CASTLE_BQ bits
    pub ep_target:    i8,  // -1 or square index 0-63
    pub side_to_move: i8,  // 1 = white, -1 = black
}

impl OnChainGame {
    /// Rebuild all bitboards from `self.board`. Called once after deserialization.
    pub fn rebuild_bitboards(&mut self) {
        self.white_pawns   = 0; self.white_knights = 0; self.white_bishops = 0;
        self.white_rooks   = 0; self.white_queens  = 0; self.white_kings   = 0;
        self.black_pawns   = 0; self.black_knights = 0; self.black_bishops = 0;
        self.black_rooks   = 0; self.black_queens  = 0; self.black_kings   = 0;
        self.occupied_white = 0; self.occupied_black = 0; self.occupied = 0;

        for sq in 0usize..64 {
            let p = self.board[sq];
            if p == 0 { continue; }
            let bit = 1u64 << sq;
            match p {
                 1 => self.white_pawns   |= bit,
                 2 => self.white_knights |= bit,
                 3 => self.white_bishops |= bit,
                 4 => self.white_rooks   |= bit,
                 5 => self.white_queens  |= bit,
                 6 => self.white_kings   |= bit,
                -1 => self.black_pawns   |= bit,
                -2 => self.black_knights |= bit,
                -3 => self.black_bishops |= bit,
                -4 => self.black_rooks   |= bit,
                -5 => self.black_queens  |= bit,
                -6 => self.black_kings   |= bit,
                _ => {}
            }
            if p > 0 { self.occupied_white |= bit; } else { self.occupied_black |= bit; }
            self.occupied |= bit;
        }
    }

    /// Incrementally set a square. Clears whatever was there before.
    #[inline]
    pub fn set_square(&mut self, sq: usize, piece: i8) {
        self.clear_square(sq);
        if piece == 0 { return; }
        self.board[sq] = piece;
        let bit = 1u64 << sq;
        match piece {
             1 => self.white_pawns   |= bit,
             2 => self.white_knights |= bit,
             3 => self.white_bishops |= bit,
             4 => self.white_rooks   |= bit,
             5 => self.white_queens  |= bit,
             6 => self.white_kings   |= bit,
            -1 => self.black_pawns   |= bit,
            -2 => self.black_knights |= bit,
            -3 => self.black_bishops |= bit,
            -4 => self.black_rooks   |= bit,
            -5 => self.black_queens  |= bit,
            -6 => self.black_kings   |= bit,
            _ => {}
        }
        if piece > 0 { self.occupied_white |= bit; } else { self.occupied_black |= bit; }
        self.occupied |= bit;
    }

    /// Remove whatever piece is on `sq`.
    #[inline]
    pub fn clear_square(&mut self, sq: usize) {
        let piece = self.board[sq];
        if piece == 0 { return; }
        self.board[sq] = 0;
        let inv = !(1u64 << sq);
        match piece {
             1 => self.white_pawns   &= inv,
             2 => self.white_knights &= inv,
             3 => self.white_bishops &= inv,
             4 => self.white_rooks   &= inv,
             5 => self.white_queens  &= inv,
             6 => self.white_kings   &= inv,
            -1 => self.black_pawns   &= inv,
            -2 => self.black_knights &= inv,
            -3 => self.black_bishops &= inv,
            -4 => self.black_rooks   &= inv,
            -5 => self.black_queens  &= inv,
            -6 => self.black_kings   &= inv,
            _ => {}
        }
        if piece > 0 { self.occupied_white &= inv; } else { self.occupied_black &= inv; }
        self.occupied &= inv;
    }

    /// Serialise back to a `CompactBoard` for storing in the account.
    pub fn to_compact_board(self) -> CompactBoard {
        CompactBoard {
            squares: self.board,
            castling: self.castling,
            ep_target: self.ep_target,
            side_to_move: self.side_to_move,
            _pad: 0,
        }
    }

    /// O(1) king square lookup via trailing_zeros.
    #[inline]
    pub fn king_square(&self, color: i8) -> Option<u8> {
        let bb = if color > 0 { self.white_kings } else { self.black_kings };
        if bb == 0 { None } else { Some(bb.trailing_zeros() as u8) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::on_chain_attack::is_in_check_fast;
    use crate::on_chain_moves;

    #[test]
    fn test_fen_roundtrip_starting_position() {
        let start_fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        let cb = CompactBoard::from_fen(start_fen);
        let out = cb.to_fen();
        assert_eq!(out, start_fen);
    }

    #[test]
    fn test_fen_roundtrip_after_e2e4() {
        let start_fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        let mut g = CompactBoard::from_fen(start_fen).to_on_chain_game();
        let _ = on_chain_moves::validate_and_apply(&mut g, b"e2e4\0").unwrap();
        let cb = g.to_compact_board();
        let out = cb.to_fen();
        assert_eq!(out, "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1");
    }
}