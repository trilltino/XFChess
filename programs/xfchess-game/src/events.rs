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

/// Audit trail for `governance_ix::recover_stuck_delegation`. Before this
/// event existed there was no on-chain record at all of who was attested as
/// white/black or how the escrow was split — see
/// docs/PRE_MAINNET_E2E_PLAN.md §1.5.
#[event]
pub struct StuckDelegationRecovered {
    pub game_id: u64,
    pub dispute_authority: Pubkey,
    pub white_authority: Pubkey,
    pub black_authority: Pubkey,
    pub white_share: u64,
    pub black_share: u64,
    pub timestamp: i64,
}
