//! Public API for the chess engine
//!
//! Provides high-level functions for game management and AI move generation

use super::types::*;
use super::constants::*;
use super::board::*;
use super::move_gen::*;
use super::search::*;

/// Helper to create empty Vec array
fn create_empty_vec_array() -> [Vec<KK>; 64] {
    [
        Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(),
        Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(),
        Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(),
        Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(),
        Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(),
        Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(),
        Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(),
        Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(),
    ]
}

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

        rook: create_empty_vec_array(),
        bishop: create_empty_vec_array(),
        knight: create_empty_vec_array(),
        king: create_empty_vec_array(),
        white_pawn: create_empty_vec_array(),
        black_pawn: create_empty_vec_array(),

        tt: Box::new([TTE::default(); TTE_SIZE]),

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

/// Get AI's response to current position
///
/// # Arguments
///
/// * `game` - The game state
/// * `color` - The color to move (1 for White, -1 for Black)
pub fn reply(game: &mut Game, color: i64) -> Move {
    find_best_move(game, game.secs_per_move, color)
}

/// Execute a move on the board
pub fn do_move(game: &mut Game, src: i8, dst: i8, update_flags: bool) -> bool {
    let piece = game.board[src as usize];

    if piece == 0 {
        return false;
    }

    // Update castling rights if needed
    if update_flags {
        match src {
            0 => game.white_rook_0_has_moved = true,
            3 => game.white_king_has_moved = true,
            7 => game.white_rook_7_has_moved = true,
            56 => game.black_rook_56_has_moved = true,
            59 => game.black_king_has_moved = true,
            63 => game.black_rook_63_has_moved = true,
            _ => {}
        }
    }

    // Execute move
    game.board[dst as usize] = piece;
    game.board[src as usize] = 0;
    game.move_counter += 1;

    true
}

/// Check if a move is legal
pub fn is_legal_move(game: &mut Game, src: i8, dst: i8, color: Color) -> bool {
    let piece = game.board[src as usize];

    if !piece_belongs_to(piece, color) {
        return false;
    }

    // Generate legal moves for this piece
    let moves = generate_pseudo_legal_moves(game, color);

    for mv in moves {
        if mv.src == src && mv.dst == dst {
            // Verify it doesn't leave king in check
            let captured = game.board[dst as usize];
            game.board[dst as usize] = piece;
            game.board[src as usize] = 0;

            let legal = !is_in_check(game, color);

            game.board[src as usize] = piece;
            game.board[dst as usize] = captured;

            return legal;
        }
    }

    false
}

/// Get current game state (playing, checkmate, stalemate)
pub fn get_game_state(game: &mut Game, color: Color) -> i32 {
    let in_check = is_in_check(game, color);
    let has_legal_moves = has_any_legal_move(game, color);

    if !has_legal_moves {
        if in_check {
            STATE_CHECKMATE
        } else {
            STATE_STALEMATE
        }
    } else {
        STATE_PLAYING
    }
}

fn has_any_legal_move(game: &mut Game, color: Color) -> bool {
    let moves = generate_pseudo_legal_moves(game, color);

    for mv in moves {
        let captured = game.board[mv.dst as usize];
        game.board[mv.dst as usize] = game.board[mv.src as usize];
        game.board[mv.src as usize] = 0;

        let legal = !is_in_check(game, color);

        game.board[mv.src as usize] = game.board[mv.dst as usize];
        game.board[mv.dst as usize] = captured;

        if legal {
            return true;
        }
    }

    false
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
