use anchor_lang::prelude::*;

#[event]
pub struct MoveEvent {
    pub game_id: u64,
    pub player: Pubkey,
    pub move_uci: [u8; 5],
    pub move_number: u16,
    pub board_state: [u8; 68],
    pub timestamp: i64,
}
