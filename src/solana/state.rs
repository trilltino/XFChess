//! Solana state types for xfchess-game program

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
