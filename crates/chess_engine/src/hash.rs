//! Zobrist hashing and transposition table
//!
//! Implements position hashing for fast position lookup and caching

use super::constants::*;
use super::types::*;

/// Simple position hash (simplified Zobrist)
pub fn position_hash(board: &Board) -> BitBuffer192 {
    let mut hash = [0u8; BIT_BUFFER_SIZE];

    // Simple hash: XOR piece values with positions
    for (i, &piece) in board.iter().enumerate() {
        if piece != 0 {
            let piece_index = (piece.abs() as usize - 1) * 2 + if piece > 0 { 0 } else { 1 };
            let hash_index = (piece_index * 64 + i) % BIT_BUFFER_SIZE;
            hash[hash_index] ^= (piece as u8).wrapping_mul(i as u8 + 1);
        }
    }

    hash
}

/// Convert hash to TT index
pub fn hash_to_index(hash: &BitBuffer192) -> usize {
    let mut index = 0usize;
    for (i, &byte) in hash.iter().take(4).enumerate() {
        index |= (byte as usize) << (i * 8);
    }
    index % TTE_SIZE
}

/// Probe transposition table (returns copy to avoid borrow issues)
pub fn tt_probe(game: &Game, hash: &BitBuffer192) -> Option<HashResult> {
    let index = hash_to_index(hash);
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
    let index = hash_to_index(&hash);
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
