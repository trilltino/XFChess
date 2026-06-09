//! Zobrist hashing and transposition table
//!
//! Implements position hashing for fast position lookup and caching

use super::constants::*;
use super::types::*;

/// Return the current pre-calculated hash
pub fn position_hash(game: &Game) -> BitBuffer192 {
    game.current_hash
}

/// Initialize the Zobrist table with random values and compute initial hash
pub fn init_zobrist(game: &mut Game) {
    let mut seed = 42u64; // Fixed seed for reproducibility
    for p in 0..12 {
        for s in 0..64 {
            for b in 0..BIT_BUFFER_SIZE {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                game.zobrist_table[p][s][b] = (seed >> 32) as u8;
            }
        }
    }
    // Initialize turn bitstring
    for b in 0..BIT_BUFFER_SIZE {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        game.zobrist_black_turn[b] = (seed >> 32) as u8;
    }
    // Compute initial hash from board state
    game.current_hash = compute_full_hash(game);
}

/// Compute a full hash from scratch (only used for initialization)
pub fn compute_full_hash(game: &Game) -> BitBuffer192 {
    let mut hash = [0u8; BIT_BUFFER_SIZE];
    for (s, &piece) in game.board.iter().enumerate() {
        if piece != 0 {
            let p_idx = piece_to_zobrist_idx(piece);
            for b in 0..BIT_BUFFER_SIZE {
                hash[b] ^= game.zobrist_table[p_idx][s][b];
            }
        }
    }
    // If it's black's turn (move_counter is odd), XOR the turn bitstring
    if game.move_counter % 2 != 0 {
        for b in 0..BIT_BUFFER_SIZE {
            hash[b] ^= game.zobrist_black_turn[b];
        }
    }
    hash
}

/// Toggle the side-to-move in the current hash
#[inline]
pub fn toggle_turn(game: &mut Game) {
    for b in 0..BIT_BUFFER_SIZE {
        game.current_hash[b] ^= game.zobrist_black_turn[b];
    }
}

/// Update the current hash by XORing a piece at a square
#[inline]
pub fn update_hash(game: &mut Game, square: usize, piece: i8) {
    if piece == 0 {
        return;
    }
    let p_idx = piece_to_zobrist_idx(piece);
    for b in 0..BIT_BUFFER_SIZE {
        game.current_hash[b] ^= game.zobrist_table[p_idx][square][b];
    }
}

/// Map a piece ID to a Zobrist table index (0-11)
#[inline]
fn piece_to_zobrist_idx(piece: i8) -> usize {
    let piece_type = piece.abs() as usize - 1; // 0-5
    let color_offset = if piece > 0 { 0 } else { 6 };
    piece_type + color_offset
}

/// Convert hash to TT index using the game's runtime capacity.
pub fn hash_to_index(hash: &BitBuffer192, capacity: usize) -> usize {
    let mut index = 0usize;
    for (i, &byte) in hash.iter().take(4).enumerate() {
        index |= (byte as usize) << (i * 8);
    }
    index % capacity
}

/// Probe transposition table (returns copy to avoid borrow issues)
pub fn tt_probe(game: &Game, hash: &BitBuffer192) -> Option<HashResult> {
    let index = hash_to_index(hash, game.tt_capacity);
    let tte = &game.tt[index];

    for entry in &tte.h {
        if entry.key == *hash && entry.res.hit > 0 {
            return Some(entry.res);
        }
    }

    None
}

/// Store position in transposition table
pub fn tt_store(game: &mut Game, hash: BitBuffer192, result: HashResult, priority: i64) {
    let index = hash_to_index(&hash, game.tt_capacity);
    let tte = &mut game.tt[index];

    // Find slot to replace (lowest priority)
    let mut min_pri_idx = 0;
    let mut min_pri = i64::MAX;

    for (i, entry) in tte.h.iter().enumerate() {
        // If same position, update it
        if entry.key == hash {
            min_pri_idx = i;
            break;
        }

        if entry.pri < min_pri {
            min_pri = entry.pri;
            min_pri_idx = i;
        }
    }

    // Store entry
    tte.h[min_pri_idx] = Guide2 {
        key: hash,
        res: result,
        pri: priority,
    };
}
