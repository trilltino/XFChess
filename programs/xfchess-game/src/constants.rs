use anchor_lang::prelude::*;

#[constant]
pub const GAME_SEED: &[u8] = b"game";
#[constant]
pub const MOVE_LOG_SEED: &[u8] = b"move_log";
#[constant]
pub const PROFILE_SEED: &[u8] = b"profile";
#[constant]
pub const WAGER_ESCROW_SEED: &[u8] = b"escrow";
#[constant]
pub const SESSION_DELEGATION_SEED: &[u8] = b"session_delegation";

// Dedicated AI Authority Pubkey (Example, to be replaced with real AI signer)
pub mod ai_authority {
    use anchor_lang::prelude::declare_id;
    declare_id!("AJwEwo74nRiZ3MPKX3XRh92rJaHj5ktPGRiY8kXhVozp");
}
