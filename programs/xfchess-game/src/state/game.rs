use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct Game {
    pub game_id: u64,
    pub white: Pubkey,
    pub black: Pubkey,
    pub status: GameStatus,
    pub result: GameResult,
    #[max_len(100)]
    pub fen: String,
    pub move_count: u16,
    pub turn: u8,
    pub created_at: i64,
    pub updated_at: i64,
    pub wager_amount: u64,
    pub wager_token: Option<Pubkey>,
    pub game_type: GameType,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct SessionDelegation {
    pub game_id: u64,
    pub player: Pubkey,
    pub session_key: Pubkey,
    pub expires_at: i64,
    pub max_batch_len: u16,
    pub enabled: bool,
    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace, Debug)]
pub enum GameStatus {
    WaitingForOpponent,
    Active,
    Inactive,
    Disputed,
    Cancelled,
    Finished,
    Expired,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace)]
pub enum GameResult {
    None,
    Winner(Pubkey),
    Draw,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace, Debug)]
pub enum GameType {
    PvP,
    PvAI,
}
