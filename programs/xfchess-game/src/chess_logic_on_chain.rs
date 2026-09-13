//! On-chain chess move validation: the `no_std` subset of chess logic compiled
//! directly into the Solana program, re-exporting `nimzovich_engine`'s no_std
//! board/move-gen types with a `validation` submodule for move-legality checks.

pub use nimzovich_engine;
pub use nimzovich_engine::{parse_uci, validate_and_apply, CompactBoard, OnChainGame};
pub use nimzovich_engine::{
    Color, Game, Move, BISHOP_ID, KING_ID, KNIGHT_ID, PAWN_ID, QUEEN_ID, ROOK_ID,
};

pub mod validation {
    use super::*;

    pub fn is_move_legal(fen_str: &str, move_uci: &str) -> bool {
        let cb = CompactBoard::from_fen(fen_str);
        let mut on_chain_game = cb.to_on_chain_game();

        let mut move_bytes = [0u8; 5];
        let bytes = move_uci.as_bytes();
        let len = bytes.len().min(5);
        move_bytes[..len].copy_from_slice(&bytes[..len]);

        validate_and_apply(&mut on_chain_game, &move_bytes).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::validation::is_move_legal;

    #[test]
    fn test_invalid_fen_returns_false() {
        assert!(!is_move_legal("not-a-fen", "e2e4"));
        assert!(!is_move_legal("PPPPPPPPP/8/8/8/8/8/8/8 w - - 0 1", "a2a3"));
    }

    #[test]
    fn test_invalid_move_format_returns_false() {
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        assert!(!is_move_legal(fen, "zzzz"));
    }

    #[test]
    fn test_known_illegal_move_returns_false() {
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        assert!(!is_move_legal(fen, "e2e5"));
    }

    #[test]
    fn test_known_legal_move_returns_true() {
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        assert!(is_move_legal(fen, "e2e4"));
    }
}
