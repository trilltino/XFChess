//! Game lifecycle management
//!
//! Functions for creating and resetting games.

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
#[cfg(all(not(feature = "std"), feature = "search"))]
use alloc::sync::Arc;


use crate::bitset::BitSet;
use crate::board::{init_bitboards, init_board};
use crate::constants::{BIT_BUFFER_SIZE, MAX_DEPTH, TTE_SIZE};
#[cfg(feature = "search")]
use crate::hash::init_zobrist;
use crate::move_gen::init_move_tables;
use crate::types::*;
use crate::utils;
#[cfg(feature = "std")]
use std::sync::{Arc, Mutex};

/// Create a new game with initial position
#[cfg(feature = "std")]
pub fn new_game() -> Game {
    let mut game = Game {
        board: init_board(),
        move_counter: 0,
        white_king_has_moved: false,
        black_king_has_moved: false,
        white_rook_0_has_moved: false,
        white_rook_7_has_moved: false,
        black_rook_56_has_moved: false,
        black_rook_63_has_moved: false,
        en_passant_target: None,
        secs_per_move: 1.5,

        rook: utils::create_empty_move_table_array(),
        bishop: utils::create_empty_move_table_array(),
        knight: utils::create_empty_move_table_array(),
        king: utils::create_empty_move_table_array(),
        white_pawn: utils::create_empty_move_table_array(),
        black_pawn: utils::create_empty_move_table_array(),

        #[cfg(feature = "search")]
        tt: Arc::new(Mutex::new(utils::create_boxed_array::<TTE, { TTE_SIZE }>())),

        #[cfg(feature = "search")]
        zobrist_table: [[[0; BIT_BUFFER_SIZE]; 64]; 12],
        #[cfg(feature = "search")]
        zobrist_black_turn: [0; BIT_BUFFER_SIZE],
        #[cfg(feature = "search")]
        current_hash: [0; BIT_BUFFER_SIZE],

        max_depth_so_far: 0,
        abs_max_depth: MAX_DEPTH as i64,
        calls: 0,
        cut: 0,
        tte_hit: 0,
        tte_put: 0,
        tte_miss: 0,
        cache_size_bytes: core::mem::size_of::<TTE>() * TTE_SIZE,

        #[cfg(feature = "search")]
        killer_moves: [[None; 2]; MAX_DEPTH + 1],
        #[cfg(feature = "search")]
        history_table: [[0; 64]; 64],
        #[cfg(feature = "search")]
        abort_search: std::sync::Arc::new(core::sync::atomic::AtomicBool::new(false)),

        // Bitboards
        white_pawns: BitSet::default(),
        white_knights: BitSet::default(),
        white_bishops: BitSet::default(),
        white_rooks: BitSet::default(),
        white_queens: BitSet::default(),
        white_kings: BitSet::default(),
        black_pawns: BitSet::default(),
        black_knights: BitSet::default(),
        black_bishops: BitSet::default(),
        black_rooks: BitSet::default(),
        black_queens: BitSet::default(),
        black_kings: BitSet::default(),
        occupied_white: BitSet::default(),
        occupied_black: BitSet::default(),
        occupied: BitSet::default(),
        sliding_attack_masks: [[BitSet::default(); 8]; 64],
    };

    #[cfg(feature = "search")]
    init_zobrist(&mut game);

    // Initialize move tables
    init_move_tables(&mut game);

    // Initialize bitboards
    init_bitboards(&mut game);

    game
}

/// Reset the game to starting position
#[cfg(feature = "std")]
pub fn reset_game(game: &mut Game) {
    game.board = init_board();
    game.move_counter = 0;
    game.white_king_has_moved = false;
    game.black_king_has_moved = false;
    game.white_rook_0_has_moved = false;
    game.white_rook_7_has_moved = false;
    game.black_rook_56_has_moved = false;
    game.black_rook_63_has_moved = false;
    game.max_depth_so_far = 0;
    game.calls = 0;
    game.cut = 0;
    game.tte_hit = 0;
    
    #[cfg(feature = "search")]
    init_zobrist(game);

    // Reset bitboards
    init_bitboards(game);

    #[cfg(feature = "search")]
    game.abort_search.store(false, core::sync::atomic::Ordering::Relaxed);
}

/// Create a game from a FEN string
#[cfg(feature = "std")]
pub fn game_from_fen(fen: &str) -> Game {
    let mut game = new_game();
    
    let parts: Vec<&str> = fen.split_whitespace().collect();
    if parts.is_empty() { return game; }

    // 1. Piece placement
    let mut board = [0i8; 64];
    let ranks: Vec<&str> = parts[0].split('/').collect();
    for (r_idx, rank_str) in ranks.iter().enumerate() {
        let rank = 7 - r_idx as u8;
        let mut file = 0u8;
        for c in rank_str.chars() {
            if let Some(digit) = c.to_digit(10) {
                file += digit as u8;
            } else {
                let sq = (rank * 8 + file) as usize;
                board[sq] = match c {
                    'P' => 1, 'N' => 2, 'B' => 3, 'R' => 4, 'Q' => 5, 'K' => 6,
                    'p' => -1, 'n' => -2, 'b' => -3, 'r' => -4, 'q' => -5, 'k' => -6,
                    _ => 0,
                };
                file += 1;
            }
        }
    }
    game.board = board;

    if parts.len() < 2 { return game; }
    
    // 2. Side to move
    // Note: game.board uses piece IDs, but move_gen uses bitboards and Turn logic.
    // We need to ensure Zobrist and Turn are synced.
    // In this engine, white moves if color is 1. 
    // We'll handle turn in the search call.

    // 3. Castling rights
    if parts.len() >= 3 {
        let castling = parts[2];
        game.white_king_has_moved = !castling.contains('K') && !castling.contains('Q');
        game.black_king_has_moved = !castling.contains('k') && !castling.contains('q');
        game.white_rook_0_has_moved = !castling.contains('Q');
        game.white_rook_7_has_moved = !castling.contains('K');
        game.black_rook_56_has_moved = !castling.contains('q');
        game.black_rook_63_has_moved = !castling.contains('k');
    }

    // 4. En Passant target square (e.g. "e3" means pawn just double-pushed)
    if parts.len() >= 4 {
        let ep_str = parts[3];
        if ep_str != "-" && ep_str.len() == 2 {
            let file = ep_str.as_bytes()[0].wrapping_sub(b'a') as i8;
            let rank = ep_str.as_bytes()[1].wrapping_sub(b'1') as i8;
            if (0..8).contains(&file) && (0..8).contains(&rank) {
                game.en_passant_target = Some(rank * 8 + file);
            }
        }
    }

    // 5. Halfmove/Fullmove
    if parts.len() >= 6 {
        game.move_counter = parts[5].parse().unwrap_or(1);
    }

    // Re-initialize bitboards and hash from the new board state
    init_bitboards(&mut game);
    #[cfg(feature = "search")]
    init_zobrist(&mut game);

    game
}