
use borsh::{BorshDeserialize, BorshSerialize};
use solana_sdk::pubkey::Pubkey;

#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum GameStatus {
    WaitingForOpponent,
    Active,
    Finished,
    Expired,
}

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
