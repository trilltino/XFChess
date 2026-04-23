//! Core state structure defining an active or historical game's properties.

use anchor_lang::prelude::*;

/// The core on-chain game account. One PDA per game_id.
/// Seeds: [b"game", game_id.to_le_bytes()]
#[account]
#[derive(InitSpace)]
pub struct Game {
    pub game_id: u64,        // Unique ID chosen by the creator (timestamp or client-generated)
    pub white: Pubkey,       // White player's wallet
    pub black: Pubkey,       // Black player's wallet (default pubkey = no opponent yet)
    pub status: GameStatus,
    pub last_move_timestamp: i64,
    pub fees_advanced: u64, // Accumulator for operational fees paid by relayer
    pub fee_payer: Pubkey, // Relayer wallet that paid; reimbursed at claim
    pub result: GameResult,
    #[max_len(100)]
    pub fen: String,         // Current board position in FEN notation
    pub move_count: u16,     // Total half-moves made
    pub turn: u8,            // Increments each half-move (1 = white to move, 2 = black, ...)
    pub created_at: i64,     // Unix timestamp
    pub updated_at: i64,     // Updated on every move — used for inactivity checks
    pub wager_amount: u64,   // Lamports each player put in (0 = casual game)
    pub wager_token: Option<Pubkey>, // None = SOL wager; Some = SPL token mint (future)
    pub game_type: GameType,
    pub match_type: MatchType, // Free, Ranked, Wager, or Tournament
    pub country_fee: u64,    // Treasury fee in lamports for this game
    pub time_per_move: i64,  // Seconds each player has per move; 0 = no time limit
    pub bump: u8,            // PDA canonical bump stored for use in signed CPI calls
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
    Expired,
    Cancelled,
}

/// Outcome recorded when a game is finalised.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace)]
pub enum GameResult {
    None,            // Game not yet finished
    Winner(Pubkey),  // The winning player's pubkey
    Draw,            // Agreed or stalemate draw
}

/// Whether this is a human vs human or human vs AI game.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace, Debug)]
pub enum GameType {
    PvP,  // Black slot stays empty until join_game is called
    PvAI, // Black is set to ai_authority::ID immediately; game is Active from creation
}

/// Match type determines fee structure and ELO impact.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace, Debug)]
pub enum MatchType {
    Free,
    Ranked,
    Wager,
    Casual,
    Tournament,
}

impl Game {
    pub const LEN: usize = 32 + 32 + 8 + 8 + 1 + 1 + 8 + 8 + 32 + 1 + 32;
}
