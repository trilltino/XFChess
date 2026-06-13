//! Zobrist hashing and transposition table.
//!
//! Positions hash to a single `u64` (standard Zobrist: one random bitstring
//! per piece/square plus a side-to-move bitstring, combined by XOR and
//! updated incrementally). This replaced a 24/32-byte byte-array scheme whose
//! per-byte XOR loops and 24-byte key compares were pure overhead — a u64
//! provides ample collision resistance for a TT with power-of-two indexing.

use super::types::*;

/// Return the current pre-calculated hash
#[inline]
pub fn position_hash(game: &Game) -> BitBuffer192 {
    game.current_hash
}

/// splitmix64 — small, fast, high-quality PRNG for table initialization.
fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

/// Initialize the Zobrist table with random values and compute initial hash
pub fn init_zobrist(game: &mut Game) {
    let mut seed = 0x5EEDC0DE_u64; // fixed seed for reproducibility
    for p in 0..12 {
        for s in 0..64 {
            game.zobrist_table[p][s] = splitmix64(&mut seed);
        }
    }
    game.zobrist_black_turn = splitmix64(&mut seed);
    game.current_hash = compute_full_hash(game);
}

/// Compute a full hash from scratch (initialization and API-level moves).
pub fn compute_full_hash(game: &Game) -> BitBuffer192 {
    let mut hash = 0u64;
    for (s, &piece) in game.board.iter().enumerate() {
        if piece != 0 {
            hash ^= game.zobrist_table[piece_to_zobrist_idx(piece)][s];
        }
    }
    // If it's black's turn (move_counter is odd), XOR the turn bitstring
    if game.move_counter % 2 != 0 {
        hash ^= game.zobrist_black_turn;
    }
    hash
}

/// Toggle the side-to-move in the current hash
#[inline]
pub fn toggle_turn(game: &mut Game) {
    game.current_hash ^= game.zobrist_black_turn;
}

/// Update the current hash by XORing a piece at a square
#[inline]
pub fn update_hash(game: &mut Game, square: usize, piece: i8) {
    if piece == 0 {
        return;
    }
    game.current_hash ^= game.zobrist_table[piece_to_zobrist_idx(piece)][square];
}

/// Map a piece ID to a Zobrist table index (0-11)
#[inline]
fn piece_to_zobrist_idx(piece: i8) -> usize {
    let piece_type = piece.abs() as usize - 1; // 0-5
    let color_offset = if piece > 0 { 0 } else { 6 };
    piece_type + color_offset
}

/// Convert hash to TT index. Capacity is always a power of two.
#[inline]
pub fn hash_to_index(hash: &BitBuffer192, capacity: usize) -> usize {
    (*hash as usize) & (capacity - 1)
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
