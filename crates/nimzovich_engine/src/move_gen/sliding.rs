//! Sliding piece move generation
//!
//! Common functionality for sliding pieces (bishops, rooks, queens).
//! These pieces can move multiple squares in a direction until blocked.
//!
//! ## Optimized Bitboard Algorithm
//!
//! This version uses precomputed sliding masks for each square and direction.
//! 1. Fetch the precomputed mask for a direction.
//! 2. Intersect with board occupancy to find blockers.
//! 3. Use bit-scanning to find the first blocker.
//! 4. Mask out squares beyond the blocker.
//! 5. Filter out own pieces on the blocker square.

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;


use crate::board::*;
use crate::types::*;

/// Generate moves for a sliding piece using precomputed bitboard masks
pub fn generate_sliding_moves(
    game: &Game,
    from: i8,
    color: Color,
    _table: &[KK],
    moves: &mut Vec<KK>,
) {
    let occupancy = game.occupied.0;
    
    // Directions: [N, S, O, W, NO, SO, SW, NW]
    // Decreasing indices: N, W, NO, NW (indices 0, 3, 4, 7)
    // Increasing indices: S, O, SO, SW (indices 1, 2, 5, 6)
    
    for dir_idx in 0..8 {
        let mask_set = game.sliding_attack_masks[from as usize][dir_idx];
        let mask = mask_set.0;
        if mask == 0 { continue; }

        let blockers = occupancy & mask;
        if blockers == 0 {
            // No blockers, all squares in mask are valid
            let mut temp_bb = mask;
            while temp_bb != 0 {
                let dst = temp_bb.trailing_zeros() as i8;
                moves.push(KK::new(from, dst, 0, 0));
                temp_bb &= temp_bb - 1;
            }
        } else {
            // Find the first blocker
            let blocker_sq = if dir_idx == 0 || dir_idx == 3 || dir_idx == 4 || dir_idx == 7 {
                // Decreasing indices: First blocker is the HIGHEST bit
                63 - blockers.leading_zeros() as i8
            } else {
                // Increasing indices: First blocker is the LOWEST bit
                blockers.trailing_zeros() as i8
            };

            // Mask for squares between 'from' and 'blocker_sq' (inclusive)
            let mut valid_mask = mask;
            if dir_idx == 0 || dir_idx == 3 || dir_idx == 4 || dir_idx == 7 {
                // Remove squares LESS than blocker_sq
                // Mask for squares >= blocker_sq: !((1 << blocker_sq) - 1)
                // Using u64 for safety
                let lower_mask = if blocker_sq == 0 { 0 } else { (1u64 << blocker_sq) - 1 };
                valid_mask &= !lower_mask;
            } else {
                // Remove squares GREATER than blocker_sq
                // Mask for squares <= blocker_sq: (1 << (blocker_sq + 1)) - 1
                let upper_mask = if blocker_sq == 63 { 0xFFFFFFFFFFFFFFFFu64 } else { (1u64 << (blocker_sq + 1)) - 1 };
                valid_mask &= upper_mask;
            }

            // Finally, filter out own pieces on the blocker square
            let mut temp_bb = valid_mask;
            while temp_bb != 0 {
                let dst = temp_bb.trailing_zeros() as i8;
                let dest_piece = game.board[dst as usize];
                if dest_piece == 0 || !piece_belongs_to(dest_piece, color) {
                    moves.push(KK::new(from, dst, 0, 0));
                }
                temp_bb &= temp_bb - 1;
            }
        }
    }
}