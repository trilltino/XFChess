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
    pub time_per_move: u16,  // Seconds each player has per move; 0 = no time limit
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
    WaitingForOpponent, // Created, wager escrowed, waiting for black to join
    Active,             // Both players present, moves are being recorded
    Inactive,           // Paused / reserved for future use
    Disputed,           // A player raised a dispute; moves frozen until resolved
    Cancelled,          // Creator cancelled before opponent joined (or both agreed)
    Finished,           // finalize_game was called and result is final
    Expired,            // No activity for 24h; wager can be reclaimed
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
    Free,      // Casual game, no fees, no ELO
    Ranked,    // Ranked game, ELO + treasury fee
    Wager,     // Wager game, ELO + treasury fee + wager
    Tournament, // Tournament game, ELO + treasury fee
}
