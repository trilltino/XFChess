use anchor_lang::prelude::*;

#[account]
pub struct MoveLog {
    pub game_id: u64,
    pub moves: Vec<String>,
}
