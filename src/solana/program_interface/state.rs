//! Solana state types for xfchess-game program
//!
//! Private Borsh-compatible mirrors of `xfchess_game` enums used by the
//! legacy `solana::multiplayer` ECS code. Not re-exported publicly.
#![allow(dead_code)]

use borsh::{BorshDeserialize, BorshSerialize};
use solana_sdk::pubkey::Pubkey;

/// Game status enum - mirrors xfchess_game::state::GameStatus
#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum GameStatus {
    WaitingForOpponent,
    Active,
    Finished,
    Expired,
}

/// Game result enum - mirrors xfchess_game::state::GameResult
#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum GameResult {
    None,
    Winner(Pubkey),
    Draw,
}

impl Default for GameStatus {
    fn default() -> Self {
        GameStatus::WaitingForOpponent
    }
}

impl Default for GameResult {
    fn default() -> Self {
        GameResult::None
    }
}
