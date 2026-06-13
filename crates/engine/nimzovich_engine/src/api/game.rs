//! Game lifecycle management
//!
//! Functions for creating and resetting games.

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use crate::bitset::BitSet;
use crate::board::{init_bitboards, init_board};
use crate::constants::{MAX_DEPTH, TTE_SIZE};
#[cfg(feature = "search")]
use crate::hash::init_zobrist;
use crate::move_gen::init_move_tables;
use crate::types::*;
use crate::utils;

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
        halfmove_clock: 0,
        secs_per_move: 1.5,

        rook: utils::create_empty_move_table_array(),
        bishop: utils::create_empty_move_table_array(),
        knight: utils::create_empty_move_table_array(),
        king: utils::create_empty_move_table_array(),
        white_pawn: utils::create_empty_move_table_array(),
        black_pawn: utils::create_empty_move_table_array(),

        #[cfg(feature = "search")]
        tt: vec![TTE::default(); TTE_SIZE],
        #[cfg(feature = "search")]
        tt_capacity: TTE_SIZE,

        #[cfg(feature = "search")]
        zobrist_table: [[0u64; 64]; 12],
        #[cfg(feature = "search")]
        zobrist_black_turn: 0,
        #[cfg(feature = "search")]
        current_hash: 0,

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
        cap_history: [[[0; 7]; 64]; 7],
        #[cfg(feature = "search")]
        conthist: [[0; 64]; 64],
        #[cfg(feature = "search")]
        abort_search: std::sync::Arc::new(core::sync::atomic::AtomicBool::new(false)),
        #[cfg(feature = "search")]
        eval_stack: [0; 128],
        #[cfg(feature = "search")]
        hash_history: Vec::new(),
        #[cfg(feature = "search")]
        search_deadline: None,

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

    // Seed the repetition history with the starting position.
    #[cfg(feature = "search")]
    game.hash_history.push(game.current_hash);

    game
}

/// Resize the transposition table to `mb` megabytes (rounded to next power of two bucket count).
///
/// Call before starting a search. Does NOT preserve existing TT entries.
#[cfg(feature = "std")]
pub fn set_tt_size_mb(game: &mut Game, mb: usize) {
    let entry_size = core::mem::size_of::<TTE>().max(1);
    let bytes = mb * 1024 * 1024;
    // Round down to power-of-two bucket count for cheap modulo in hash_to_index
    let raw = (bytes / entry_size).max(1);
    let capacity = raw.next_power_of_two() / 2;  // floor to previous power of two
    let capacity = capacity.max(1);

    game.tt.resize_with(capacity, TTE::default);
    game.tt.shrink_to_fit();

    game.tt_capacity = capacity;
    game.cache_size_bytes = entry_size * capacity;
    eprintln!("[TT] Resized to {} entries ({} MB)", capacity, capacity * entry_size / (1024 * 1024));
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
    game.halfmove_clock = 0;
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
    set_game_from_fen(&mut game, fen);
    game
}

/// Update an existing game's position from a FEN string, reusing the TT allocation.
///
/// Use this instead of `game_from_fen` when you already hold a `Game` and want to avoid
/// the 2M-entry TTE reallocation (~2.2 GB write) that `game_from_fen` triggers via `new_game`.
#[cfg(feature = "std")]
pub fn set_game_from_fen(game: &mut Game, fen: &str) {
    let parts: Vec<&str> = fen.split_whitespace().collect();
    if parts.is_empty() { return; }

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

    // 2. Castling rights
    if parts.len() >= 3 {
        let castling = parts[2];
        game.white_king_has_moved = !castling.contains('K') && !castling.contains('Q');
        game.black_king_has_moved = !castling.contains('k') && !castling.contains('q');
        game.white_rook_0_has_moved = !castling.contains('Q');
        game.white_rook_7_has_moved = !castling.contains('K');
        game.black_rook_56_has_moved = !castling.contains('q');
        game.black_rook_63_has_moved = !castling.contains('k');
    }

    // 3. En Passant target square (e.g. "e3" means pawn just double-pushed)
    game.en_passant_target = None;
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

    // 4. Halfmove clock
    if parts.len() >= 5 {
        game.halfmove_clock = parts[4].parse().unwrap_or(0);
    }

    // 5. Fullmove counter → move_counter
    if parts.len() >= 6 {
        game.move_counter = (parts[5].parse::<i32>().unwrap_or(1) - 1) * 2;
        if parts.len() >= 2 && parts[1] == "b" {
            game.move_counter += 1;
        }
    }

    // Sync bitboards and hash from the new board state
    init_bitboards(game);
    #[cfg(feature = "search")]
    {
        init_zobrist(game);
        // Fresh position: repetition history restarts here.
        game.hash_history.clear();
        game.hash_history.push(game.current_hash);
    }
}

/// Generate a FEN string from the current game state.
#[cfg(feature = "std")]
pub fn game_to_fen(game: &Game) -> String {
    // 1. Piece placement
    let mut ranks = Vec::with_capacity(8);
    for rank in (0..8u8).rev() {
        let mut rank_str = String::new();
        let mut empty = 0u8;
        for file in 0..8u8 {
            let sq = (rank * 8 + file) as usize;
            let id = game.board[sq];
            if id == 0 {
                empty += 1;
            } else {
                if empty > 0 {
                    rank_str.push((b'0' + empty) as char);
                    empty = 0;
                }
                let ch = match id.abs() {
                    1 => 'p',
                    2 => 'n',
                    3 => 'b',
                    4 => 'r',
                    5 => 'q',
                    6 => 'k',
                    _ => '?',
                };
                rank_str.push(if id > 0 { ch.to_ascii_uppercase() } else { ch });
            }
        }
        if empty > 0 {
            rank_str.push((b'0' + empty) as char);
        }
        ranks.push(rank_str);
    }
    let piece_placement = ranks.join("/");

    // 2. Side to move
    let side = if game.move_counter % 2 == 0 { 'w' } else { 'b' };

    // 3. Castling rights
    let mut castling = String::new();
    if !game.white_king_has_moved && !game.white_rook_7_has_moved { castling.push('K'); }
    if !game.white_king_has_moved && !game.white_rook_0_has_moved { castling.push('Q'); }
    if !game.black_king_has_moved && !game.black_rook_63_has_moved { castling.push('k'); }
    if !game.black_king_has_moved && !game.black_rook_56_has_moved { castling.push('q'); }
    if castling.is_empty() { castling.push('-'); }

    // 4. En passant target
    let ep = match game.en_passant_target {
        Some(sq) => {
            let file = (sq % 8) as u8;
            let rank = (sq / 8) as u8;
            format!("{}{}", (b'a' + file) as char, (b'1' + rank) as char)
        }
        None => "-".to_string(),
    };

    // 5. Halfmove clock
    let halfmove = game.halfmove_clock;

    // 6. Fullmove counter
    let fullmove = game.move_counter / 2 + 1;

    format!(
        "{} {} {} {} {} {}",
        piece_placement, side, castling, ep, halfmove, fullmove
    )
}