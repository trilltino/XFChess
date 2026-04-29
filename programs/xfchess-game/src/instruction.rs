use anchor_lang::prelude::*;
use crate::constants::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
#[instruction]
pub struct CreateGame {
    pub game_id: u64,
    pub wager_amount: u64,
    pub match_type: crate::state::game::MatchType,
    pub country: String,
    pub base_time_seconds: u64,
    pub increment_seconds: u16,
}

impl CreateGame {
    pub fn data(&self) -> Vec<u8> {
        self.try_to_vec().unwrap()
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
#[instruction]
pub struct TimeControl {
    pub initial_time: u32,
    pub increment: u32,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
#[instruction]
pub struct JoinGame {
    pub game_id: u64,
}

impl JoinGame {
    pub fn data(&self) -> Vec<u8> {
        self.try_to_vec().unwrap()
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
#[instruction]
pub struct RecordMove {
    pub game_id: u64,
    pub move_str: String,
    pub next_fen: String,
    pub nonce: u64,
    pub signature: Option<Vec<u8>>,
}

impl RecordMove {
    pub fn data(&self) -> Vec<u8> {
        self.try_to_vec().unwrap()
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
#[instruction]
pub struct Finalize {
    pub game_id: u64,
    pub result: GameResult,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub enum GameResult {
    Winner(Pubkey),
    Draw,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
#[instruction]
pub struct ResolveDispute {
    pub game_id: u64,
    pub resolution: String,
    pub winner: Option<Pubkey>,
}

impl ResolveDispute {
    pub fn data(&self) -> Vec<u8> {
        self.try_to_vec().unwrap()
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
#[instruction]
pub struct InitializeTournament {
    pub tournament_id: u64,
    pub name: String,
    pub entry_fee: u64,
    pub prize_pool: u64,
    pub max_players: u16,
    pub tournament_type: crate::state::tournament::TournamentType,
    pub elo_min: u32,
    pub elo_max: u32,
    pub min_players: u16,
    pub prize_shares: [u16; 10],
    pub winner_takes_all: bool,
    pub host_treasury: Pubkey,
    pub usdc_mint: Option<Pubkey>,
    pub base_time_seconds: u64,
    pub increment_seconds: u16,
}

impl InitializeTournament {
    pub fn data(&self) -> Vec<u8> {
        self.try_to_vec().unwrap()
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
#[instruction]
pub struct RegisterPlayer {
    pub tournament_id: u64,
    pub elo: u32,
}

impl RegisterPlayer {
    pub fn data(&self) -> Vec<u8> {
        self.try_to_vec().unwrap()
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct CloseTournament {
    pub tournament_id: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct ClaimPrize {
    pub tournament_id: u64,
    pub position: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
#[instruction]
pub struct FundUsdcPrize {
    pub tournament_id: u64,
    pub amount: u64,
}

impl FundUsdcPrize {
    pub fn data(&self) -> Vec<u8> {
        self.try_to_vec().unwrap()
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, PartialEq, Eq)]
#[instruction]
pub struct SessionCreateGame {
    pub tournament_id: u64,
    pub game_id: u64,
    pub wager_amount: u64,
    pub match_type: crate::state::game::MatchType,
    pub country: String,
    pub base_time_seconds: u64,
    pub increment_seconds: u16,
}

impl SessionCreateGame {
    pub fn data(&self) -> Vec<u8> {
        self.try_to_vec().unwrap()
    }
}
