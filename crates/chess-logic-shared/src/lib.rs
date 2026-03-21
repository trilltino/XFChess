#![no_std]

extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

pub use shakmaty;
pub use shakmaty::{Board, Chess, Color, Move, Piece, Role, Square, Position, Bitboard, CastlingMode};
pub use shakmaty::fen::Fen;

#[cfg(feature = "uci")]
pub use shakmaty::uci::UciMove as Uci;

/// Re-export commonly used types and functions for move validation
pub mod validation {
    use super::*;
    use alloc::string::String;
    
    /// Parses a FEN and move string to validate if the move is legal
    pub fn is_move_legal(fen_str: &str, move_uci: &str) -> bool {
        let fen: Fen = match fen_str.parse() {
            Ok(f) => f,
            Err(_) => return false,
        };
        
        let pos: Chess = match fen.into_position(shakmaty::CastlingMode::Standard) {
            Ok(p) => p,
            Err(_) => return false,
        };
        
        // Use UCI parsing if available, otherwise simplified parsing
        #[cfg(feature = "uci")]
        {
            if let Ok(m) = move_uci.parse::<Uci>() {
                if let Ok(mov) = m.to_move(&pos) {
                    return pos.is_legal(mov);
                }
            }
        }
        
        // Basic fallback for no_std/no_uci environments if needed
        // (In our case, we'll try to keep uci available where possible)
        false
    }
}
