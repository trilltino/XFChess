//! Game lifecycle management
//!
//! Functions for creating and resetting games.

use crate::board::*;
use crate::constants::{MAX_DEPTH, TTE_SIZE};
use crate::move_gen::init_move_tables;
use crate::types::*;
use crate::utils;
use std::sync::{Arc, Mutex};

/// Create a new game with initial position
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
        secs_per_move: 1.5,

        rook: utils::create_empty_move_table_array(),
        bishop: utils::create_empty_move_table_array(),
        knight: utils::create_empty_move_table_array(),
        king: utils::create_empty_move_table_array(),
        white_pawn: utils::create_empty_move_table_array(),
        black_pawn: utils::create_empty_move_table_array(),

        // Allocate transposition table directly on heap to avoid stack overflow
        // The transposition table is 2M entries, which would overflow the stack
        // if allocated normally. This helper function safely allocates on the heap.
        // Wrapped in Arc<Mutex> to allow shared access without huge clones stuttering gameplay.
        tt: Arc::new(Mutex::new(utils::create_boxed_array::<TTE, { TTE_SIZE }>())),

        max_depth_so_far: 0,
        abs_max_depth: MAX_DEPTH as i64,
        calls: 0,
        cut: 0,
        tte_hit: 0,
        tte_put: 0,
        tte_miss: 0,
        cache_size_bytes: std::mem::size_of::<TTE>() * TTE_SIZE,
    };

    // Initialize move tables
    init_move_tables(&mut game);

    game
}

/// Reset the game to starting position
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
}
