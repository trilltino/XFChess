#![no_std]

extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

pub use nimzovich_engine;
pub use nimzovich_engine::{Game, Move, Color, BISHOP_ID, KING_ID, KNIGHT_ID, PAWN_ID, QUEEN_ID, ROOK_ID};
pub use nimzovich_engine::{OnChainGame, CompactBoard, validate_and_apply, parse_uci};

/// Re-export commonly used types and functions for move validation
pub mod validation {
    use super::*;
    
    /// Parses a FEN and move string to validate if the move is legal
    pub fn is_move_legal(fen_str: &str, move_uci: &str) -> bool {
        // Use the on-chain optimized validation
        let cb = CompactBoard::from_fen(fen_str);
        let mut on_chain_game = cb.to_on_chain_game();
        
        // Parse UCI move
        let mut move_bytes = [0u8; 5];
        let bytes = move_uci.as_bytes();
        let len = bytes.len().min(5);
        move_bytes[..len].copy_from_slice(&bytes[..len]);
        
        // Validate and apply
        validate_and_apply(&mut on_chain_game, &move_bytes).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::validation::is_move_legal;

    #[test]
    fn test_invalid_fen_returns_false() {
        // is_move_legal must handle bad FEN gracefully (no panic) and reject the move.
        assert!(!is_move_legal("not-a-fen", "e2e4"));
        // An over-full rank must not panic the FEN parser.
        assert!(!is_move_legal("PPPPPPPPP/8/8/8/8/8/8/8 w - - 0 1", "a2a3"));
    }

    #[test]
    fn test_invalid_move_format_returns_false() {
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        assert!(!is_move_legal(fen, "zzzz"));
    }

    #[test]
    fn test_known_illegal_move_returns_false() {
        // e2e5 is not a legal pawn move from start
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        assert!(!is_move_legal(fen, "e2e5"));
    }
    
    #[test]
    fn test_known_legal_move_returns_true() {
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        assert!(is_move_legal(fen, "e2e4"));
    }
}
