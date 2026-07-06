//! Core state structure defining an active or historical game's properties.

use anchor_lang::prelude::*;

/// The core on-chain game account. One PDA per game_id.
/// Seeds: [b"game", game_id.to_le_bytes()]
#[account]
#[derive(InitSpace)]
pub struct Game {
    pub game_id: u64,  // Unique ID chosen by the creator (timestamp or client-generated)
    pub white: Pubkey, // White player's wallet
    pub black: Pubkey, // Black player's wallet (default pubkey = no opponent yet)
    pub status: GameStatus,
    pub last_move_timestamp: i64,
    pub fees_advanced: u64, // Accumulator for operational fees paid by relayer
    pub fee_payer: Pubkey,  // Relayer wallet that paid; reimbursed at claim
    pub result: GameResult,
    pub board_state: [u8; 68], // Compact binary form of the game (replaces FEN string)
    pub move_count: u16,       // Total half-moves made
    pub halfmove_clock: u16, // Half-moves since the last pawn move or capture (50-move rule = 100)
    pub turn: u16, // Increments each half-move (odd = white to move, even = black). u16: a u8 would overflow-panic past half-move 255 in a long game.
    pub created_at: i64, // Unix timestamp
    pub updated_at: i64, // Updated on every move — used for inactivity checks
    pub wager_amount: u64, // Lamports each player put in (0 = casual game)
    pub wager_token: Option<Pubkey>, // None = SOL wager; Some = SPL token mint (future)
    pub game_type: GameType,
    pub match_type: MatchType,  // Free, Ranked, Wager, or Tournament
    pub country_fee: u64,       // Treasury fee in lamports for this game
    pub base_time_seconds: u64, // Total clock per player in seconds; 0 = no time limit
    pub increment_seconds: u16, // Fischer increment added after each move
    pub bump: u8,               // PDA canonical bump stored for use in signed CPI calls
    pub is_delegated: bool,     // True once delegate_game is called; false after undelegate
    pub tournament_id: Option<u64>,
    pub nonce: u64, // Counter for replay protection
}

/// Short-lived delegation allowing a VPS session key to submit moves on behalf
/// of a player without requiring a wallet popup each time.
/// Seeds: [b"session_delegation", game_id.to_le_bytes(), player_pubkey]
#[account]
#[derive(InitSpace)]
pub struct SessionDelegation {
    pub game_id: u64,
    pub player: Pubkey,      // The real wallet this delegation is for
    pub session_key: Pubkey, // The hot key allowed to sign moves (held by the VPS)
    pub expires_at: i64,     // Session becomes invalid after this timestamp
    pub max_batch_len: u16,  // Max moves allowed per commit_move_batch call
    pub enabled: bool,       // Can be set false by revoke_session_key
    pub bump: u8,
}

/// Lifecycle state of a game.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace, Debug)]
pub enum GameStatus {
    Pending,
    WaitingForOpponent,
    Active,
    Inactive,
    Disputed,
    Finished,
    Settled,
    Expired,
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GamePhase {
    WaitingBase,
    ActiveBase,
    ActiveEr,
    FinishedEr,
    FinishedBase,
    Settled,
    Cancelled,
    Disputed,
}

/// Outcome recorded when a game is finalised.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq, InitSpace)]
pub enum GameResult {
    None,           // Game not yet finished
    Winner(Pubkey), // The winning player's pubkey
    Draw,           // Agreed or stalemate draw
}

/// Match variant — always player vs player; AI games are handled off-chain.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace, Debug)]
pub enum GameType {
    PvP,
}

/// Match type determines fee structure and ELO impact.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq, InitSpace)]
pub enum MatchType {
    Free,
    Rated,
}

impl Game {
    pub const LEGACY_LEN_DO_NOT_USE: usize = 32 + 32 + 8 + 8 + 1 + 1 + 8 + 8 + 32 + 1 + 32 + 8;

    pub fn phase(&self) -> Result<GamePhase> {
        match (self.status, self.is_delegated) {
            (GameStatus::WaitingForOpponent, false) => Ok(GamePhase::WaitingBase),
            (GameStatus::Active, false) => Ok(GamePhase::ActiveBase),
            (GameStatus::Active, true) => Ok(GamePhase::ActiveEr),
            (GameStatus::Finished, true) => Ok(GamePhase::FinishedEr),
            (GameStatus::Finished, false) => Ok(GamePhase::FinishedBase),
            (GameStatus::Settled, false) => Ok(GamePhase::Settled),
            (GameStatus::Cancelled, false) => Ok(GamePhase::Cancelled),
            (GameStatus::Disputed, _) => Ok(GamePhase::Disputed),
            _ => err!(crate::errors::GameErrorCode::InvalidLifecycleTransition),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(status: GameStatus, is_delegated: bool) -> Game {
        Game {
            game_id: 1,
            white: Pubkey::new_unique(),
            black: Pubkey::new_unique(),
            status,
            last_move_timestamp: 0,
            fees_advanced: 0,
            fee_payer: Pubkey::new_unique(),
            result: GameResult::None,
            board_state: [0; 68],
            move_count: 0,
            halfmove_clock: 0,
            turn: 1,
            created_at: 0,
            updated_at: 0,
            wager_amount: 0,
            wager_token: None,
            game_type: GameType::PvP,
            match_type: MatchType::Free,
            country_fee: 0,
            base_time_seconds: 0,
            increment_seconds: 0,
            bump: 0,
            is_delegated,
            tournament_id: None,
            nonce: 0,
        }
    }

    #[test]
    fn derives_base_and_er_phases() {
        assert_eq!(
            game(GameStatus::WaitingForOpponent, false).phase().unwrap(),
            GamePhase::WaitingBase
        );
        assert_eq!(
            game(GameStatus::Active, false).phase().unwrap(),
            GamePhase::ActiveBase
        );
        assert_eq!(
            game(GameStatus::Active, true).phase().unwrap(),
            GamePhase::ActiveEr
        );
        assert_eq!(
            game(GameStatus::Finished, true).phase().unwrap(),
            GamePhase::FinishedEr
        );
        assert_eq!(
            game(GameStatus::Finished, false).phase().unwrap(),
            GamePhase::FinishedBase
        );
    }

    #[test]
    fn rejects_invalid_delegated_waiting_phase() {
        assert!(game(GameStatus::WaitingForOpponent, true).phase().is_err());
    }
}
