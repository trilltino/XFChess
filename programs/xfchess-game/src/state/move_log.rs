//! Account struct designed to track the chronological sequence of FENs/moves.

use anchor_lang::prelude::*;

#[account]
pub struct MoveLog {
    pub game_id: u64,
    pub moves: Vec<String>,
    pub timestamps: Vec<i64>,
    pub player_signatures: Vec<Vec<u8>>,
    pub nonce: u64,
}

impl MoveLog {
    pub const LEN: usize = 8 + 8 + 4 + (300 * (32 + 8 + 64));
}
