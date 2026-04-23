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

#[cfg(test)]
mod tests {
    use super::validation::is_move_legal;

    #[test]
    fn test_e2e4_legal_from_start() {
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        assert!(is_move_legal(fen, "e2e4"));
    }

    #[test]
    fn test_e2e5_illegal_from_start() {
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        assert!(!is_move_legal(fen, "e2e5"));
    }

    #[test]
    fn test_invalid_fen_returns_false() {
        assert!(!is_move_legal("not-a-fen", "e2e4"));
    }

    #[test]
    fn test_invalid_move_format_returns_false() {
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        assert!(!is_move_legal(fen, "zzzz"));
    }

    #[test]
    fn test_black_legal_move_from_start() {
        let fen = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1";
        assert!(is_move_legal(fen, "e7e5"));
    }
}
