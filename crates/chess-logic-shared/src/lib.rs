#![no_std]

extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

use nimzovich_engine::{game_from_fen, is_legal_move, parse_uci};

/// Re-export commonly used types and functions for move validation
pub mod validation {
    use super::*;
    
    /// Parses a FEN and move string to validate if the move is legal
    pub fn is_move_legal(fen_str: &str, move_uci: &str) -> bool {
        let game = game_from_fen(fen_str);
        
        let mut uci_bytes = [0u8; 5];
        let bytes = move_uci.as_bytes();
        let len = bytes.len().min(5);
        uci_bytes[..len].copy_from_slice(&bytes[..len]);
        
        if let Ok((src, dst, promo)) = parse_uci(&uci_bytes) {
            let side = if fen_str.contains(" w ") { 1 } else { -1 };
            let mut game_copy = game;
            // Note: is_legal_move doesn't currently use the promo arg for check detection,
            // but it's passed for completeness if we decide to add full move simulation there.
            return is_legal_move(&mut game_copy, src, dst, side);
        }
        
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
    fn test_black_legal_move_from_start() {
        let fen = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1";
        assert!(is_move_legal(fen, "e7e5"));
    }
}
