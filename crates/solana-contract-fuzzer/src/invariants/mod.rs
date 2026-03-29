//! Invariant checking for fuzzing
//!
//! Property checks that must hold after each instruction execution.

use anyhow::Result;
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;

/// Tracks game state for invariant checking
#[derive(Debug, Default)]
pub struct InvariantChecker {
    /// Known games and their states
    pub games: std::collections::HashMap<u64, GameState>,
    /// Violations found
    pub violations: Vec<InvariantViolation>,
}

#[derive(Debug, Clone)]
pub struct GameState {
    pub game_id: u64,
    pub status: GameStatus,
    pub white: Option<Pubkey>,
    pub black: Option<Pubkey>,
    pub turn: u8, // 0 = white, 1 = black
    pub move_count: u32,
    pub wager: u64,
}

#[derive(Debug, Clone)]
pub enum GameStatus {
    Waiting,
    Active,
    Finished,
    Expired,
}

#[derive(Debug, Clone)]
pub struct InvariantViolation {
    pub game_id: u64,
    pub invariant: String,
    pub details: String,
}

impl InvariantChecker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Check all invariants after an instruction
    pub fn check_invariants(&mut self, game_id: u64, rpc: &RpcClient) -> Result<Vec<InvariantViolation>> {
        let mut violations = Vec::new();

        // Fetch game state from chain
        let game_account = match self.fetch_game_state(game_id, rpc) {
            Some(state) => state,
            None => return Ok(violations), // Game doesn't exist yet
        };

        // Invariant 1: Game status must be valid
        if !self.is_valid_status(&game_account.status) {
            violations.push(InvariantViolation {
                game_id,
                invariant: "valid_status".to_string(),
                details: format!("Invalid status: {:?}", game_account.status),
            });
        }

        // Invariant 2: If game is active, both players must be set
        if matches!(game_account.status, GameStatus::Active) {
            if game_account.white.is_none() || game_account.black.is_none() {
                violations.push(InvariantViolation {
                    game_id,
                    invariant: "active_has_both_players".to_string(),
                    details: "Active game missing players".to_string(),
                });
            }
        }

        // Invariant 3: Move count should match turn
        let expected_turn = game_account.move_count % 2;
        if game_account.turn != expected_turn as u8 {
            violations.push(InvariantViolation {
                game_id,
                invariant: "turn_matches_move_count".to_string(),
                details: format!(
                    "Turn {} doesn't match move count {}",
                    game_account.turn, game_account.move_count
                ),
            });
        }

        // Invariant 4: Finished game should have winner or draw
        if matches!(game_account.status, GameStatus::Finished) {
            // Check that ELO was updated - this is a soft check
            // We'd need to fetch player profiles to verify
        }

        // Store state for next check
        self.games.insert(game_id, game_account);
        self.violations.extend(violations.clone());

        Ok(violations)
    }

    fn fetch_game_state(&self, game_id: u64, _rpc: &RpcClient) -> Option<GameState> {
        // TODO: Actually fetch from chain using game PDA
        // For now return None to skip checks
        None
    }

    fn is_valid_status(&self, status: &GameStatus) -> bool {
        matches!(status, GameStatus::Waiting | GameStatus::Active | GameStatus::Finished | GameStatus::Expired)
    }

    /// Check economic invariants
    pub fn check_economic_invariants(
        &self,
        _game_id: u64,
        _escrow_balance: u64,
    ) -> Result<Vec<InvariantViolation>> {
        let violations = Vec::new();

        // TODO: Check escrow matches wager
        // TODO: Check no double-spend
        // TODO: Check rent exemption

        Ok(violations)
    }
}

/// Specific invariant check functions
pub mod checks {
    use super::*;

    /// Check turn alternation
    pub fn check_turn_alternation(prev_turn: u8, new_turn: u8) -> Option<InvariantViolation> {
        if new_turn == prev_turn {
            Some(InvariantViolation {
                game_id: 0, // Set by caller
                invariant: "turn_alternation".to_string(),
                details: format!("Turn didn't alternate: {} -> {}", prev_turn, new_turn),
            })
        } else {
            None
        }
    }

    /// Check authorization
    pub fn check_authorization(signer: Pubkey, expected: Pubkey) -> Option<InvariantViolation> {
        if signer != expected {
            Some(InvariantViolation {
                game_id: 0,
                invariant: "authorization".to_string(),
                details: format!("Unauthorized: {} expected {}", signer, expected),
            })
        } else {
            None
        }
    }
}
