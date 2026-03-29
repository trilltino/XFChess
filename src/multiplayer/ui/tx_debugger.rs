//! Transaction Debugger - Terminal-based rollup transaction monitoring
//!
//! This module provides real-time logging of ephemeral rollup transactions
//! to stdout (with colors) and optionally to a file.
//!
//! # Usage
//!
//! ## As a sidecar process:
//! ```bash
//! ./xfchess-debugger --game-id 12345 --log-file ./game.log
//! ```
//!
//! ## Within the game process:
//! ```rust
//! let debugger = TransactionDebugger::new(Some(file), true);
//! debugger.start_listening(rollup_events);
//! ```

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{self, Write};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(feature = "solana")]
use crate::multiplayer::rollup::manager::RollupEvent;

/// Status of a rollup transaction
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TransactionStatus {
    Pending,
    Confirmed,
    Failed,
}

/// Type of transaction event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransactionType {
    BatchProposed,
    BatchAccepted,
    BatchRejected,
    SolanaSubmitted,
    SolanaConfirmed,
    SolanaFailed,
    P2PMessageReceived,
    P2PMessageSent,
}

/// A recorded rollup transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollupTransaction {
    pub timestamp: u64,
    pub game_id: u64,
    pub tx_type: TransactionType,
    pub status: TransactionStatus,
    pub batch_hash: String,
    pub moves: Vec<String>,
    pub solana_signature: Option<String>,
    pub error: Option<String>,
    pub p2p_message: Option<P2PMessage>,
}

/// P2P message metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P2PMessage {
    pub message_type: String,
    pub from: String,
    pub timestamp: u64,
}

/// Resource that tracks all rollup transactions
#[derive(Resource)]
pub struct TransactionDebugger {
    pub transactions: Vec<RollupTransaction>,
    pub log_file: Option<File>,
    pub pretty_print: bool,
    pub game_id: Option<u64>,
}

impl TransactionDebugger {
    /// Create a new transaction debugger
    ///
    /// # Arguments
    /// * `log_file` - Optional file to write JSON logs to
    /// * `pretty_print` - Whether to print colored output to stdout
    /// * `game_id` - Optional game ID to filter events
    pub fn new(log_file: Option<File>, pretty_print: bool, game_id: Option<u64>) -> Self {
        if pretty_print {
            println!("\x1b[36m[TX Debugger]\x1b[0m Initialized");
            if let Some(id) = game_id {
                println!("\x1b[36m[TX Debugger]\x1b[0m Monitoring game ID: {}", id);
            }
            if log_file.is_some() {
                println!("\x1b[36m[TX Debugger]\x1b[0m Logging to file enabled");
            }
        }

        Self {
            transactions: Vec::new(),
            log_file,
            pretty_print,
            game_id,
        }
    }

    /// Log a new transaction event
    pub fn log_event(&mut self, tx: RollupTransaction) {
        // Filter by game_id if specified
        if let Some(filter_id) = self.game_id {
            if tx.game_id != filter_id {
                return;
            }
        }

        // Print to stdout with colors
        if self.pretty_print {
            println!("{}", self.format_colored(&tx));
        }

        // Write to log file as JSON
        if let Some(ref mut file) = self.log_file {
            let json = serde_json::to_string(&tx).unwrap_or_default();
            writeln!(file, "{}", json).ok();
            file.flush().ok();
        }

        self.transactions.push(tx);
    }

    /// Log a rollup event
    #[cfg(feature = "solana")]
    pub fn log_rollup_event(&mut self, event: &RollupEvent) {
        let timestamp = current_timestamp();

        match event {
            RollupEvent::BatchReady {
                game_id,
                moves,
                next_fens,
            } => {
                let batch_hash = calculate_batch_hash(*game_id, moves, next_fens);
                self.log_event(RollupTransaction {
                    timestamp,
                    game_id: *game_id,
                    tx_type: TransactionType::BatchProposed,
                    status: TransactionStatus::Pending,
                    batch_hash,
                    moves: moves.clone(),
                    solana_signature: None,
                    error: None,
                    p2p_message: None,
                });
            }
            RollupEvent::BatchCommitted {
                game_id,
                new_fen,
                new_turn,
            } => {
                self.log_event(RollupTransaction {
                    timestamp,
                    game_id: *game_id,
                    tx_type: TransactionType::SolanaConfirmed,
                    status: TransactionStatus::Confirmed,
                    batch_hash: String::new(), // Would need to track this from BatchReady
                    moves: Vec::new(),
                    solana_signature: None, // Would need to track from submission
                    error: None,
                    p2p_message: None,
                });

                if self.pretty_print {
                    println!(
                        "\x1b[32m[✓]\x1b[0m Game {} updated | Turn: {} | FEN: {}...",
                        game_id,
                        new_turn,
                        &new_fen[..20.min(new_fen.len())]
                    );
                }
            }
            RollupEvent::BatchFailed {
                game_id,
                moves,
                next_fens,
            } => {
                let batch_hash = calculate_batch_hash(*game_id, moves, next_fens);
                self.log_event(RollupTransaction {
                    timestamp,
                    game_id: *game_id,
                    tx_type: TransactionType::SolanaFailed,
                    status: TransactionStatus::Failed,
                    batch_hash,
                    moves: moves.clone(),
                    solana_signature: None,
                    error: Some("Batch commit failed".to_string()),
                    p2p_message: None,
                });
            }
            RollupEvent::GameEndBatch {
                game_id,
                moves,
                next_fens: _,
            } => {
                let batch_hash = calculate_batch_hash(*game_id, moves, &[]);
                self.log_event(RollupTransaction {
                    timestamp,
                    game_id: *game_id,
                    tx_type: TransactionType::BatchProposed,
                    status: TransactionStatus::Pending,
                    batch_hash,
                    moves: moves.clone(),
                    solana_signature: None,
                    error: None,
                    p2p_message: None,
                });
            }
            RollupEvent::NeedResync { game_id } => {
                if self.pretty_print {
                    println!("\x1b[33m[!]\x1b[0m Game {} needs resync", game_id);
                }
            }
        }
    }

    /// Log a P2P message
    pub fn log_p2p_message(&mut self, game_id: u64, message_type: &str, from: &str) {
        let timestamp = current_timestamp();

        self.log_event(RollupTransaction {
            timestamp,
            game_id,
            tx_type: TransactionType::P2PMessageReceived,
            status: TransactionStatus::Confirmed,
            batch_hash: String::new(),
            moves: Vec::new(),
            solana_signature: None,
            error: None,
            p2p_message: Some(P2PMessage {
                message_type: message_type.to_string(),
                from: from.to_string(),
                timestamp,
            }),
        });
    }

    /// Log Solana transaction submission
    pub fn log_solana_submission(&mut self, game_id: u64, signature: &str, batch_hash: &str) {
        let timestamp = current_timestamp();

        self.log_event(RollupTransaction {
            timestamp,
            game_id,
            tx_type: TransactionType::SolanaSubmitted,
            status: TransactionStatus::Pending,
            batch_hash: batch_hash.to_string(),
            moves: Vec::new(),
            solana_signature: Some(signature.to_string()),
            error: None,
            p2p_message: None,
        });

        if self.pretty_print {
            println!(
                "\x1b[36m[→]\x1b[0m Submitted to Solana | Sig: {}... | Game: {}",
                &signature[..16.min(signature.len())],
                game_id
            );
        }
    }

    /// Format transaction for colored terminal output
    fn format_colored(&self, tx: &RollupTransaction) -> String {
        match (&tx.status, &tx.tx_type) {
            (TransactionStatus::Confirmed, TransactionType::SolanaConfirmed) => {
                format!(
                    "\x1b[32m[✓ CONFIRMED]\x1b[0m Game {} | Batch: {}... | Moves: {}",
                    tx.game_id,
                    &tx.batch_hash[..8.min(tx.batch_hash.len())],
                    tx.moves.len()
                )
            }
            (TransactionStatus::Confirmed, TransactionType::BatchAccepted) => {
                format!(
                    "\x1b[32m[✓ ACCEPTED]\x1b[0m Game {} | Batch: {}...",
                    tx.game_id,
                    &tx.batch_hash[..8.min(tx.batch_hash.len())]
                )
            }
            (TransactionStatus::Failed, _) => {
                format!(
                    "\x1b[31m[✗ FAILED]\x1b[0m Game {} | Batch: {}... | Error: {}",
                    tx.game_id,
                    &tx.batch_hash[..8.min(tx.batch_hash.len())],
                    tx.error.as_ref().unwrap_or(&"Unknown".to_string())
                )
            }
            (TransactionStatus::Pending, TransactionType::BatchProposed) => {
                format!(
                    "\x1b[33m[⟳ PROPOSED]\x1b[0m Game {} | Batch: {}... | Moves: {}",
                    tx.game_id,
                    &tx.batch_hash[..8.min(tx.batch_hash.len())],
                    tx.moves.len()
                )
            }
            (TransactionStatus::Pending, TransactionType::SolanaSubmitted) => {
                format!(
                    "\x1b[36m[→ SUBMITTED]\x1b[0m Game {} | Sig: {}...",
                    tx.game_id,
                    tx.solana_signature
                        .as_ref()
                        .unwrap_or(&"N/A".to_string())
                        .get(..16)
                        .unwrap_or("")
                )
            }
            _ => {
                format!(
                    "\x1b[90m[•]\x1b[0m Game {} | {:?} | {:?}",
                    tx.game_id, tx.tx_type, tx.status
                )
            }
        }
    }

    /// Export all transactions to a JSON file
    pub fn export_to_json(&self, path: &PathBuf) -> io::Result<()> {
        let json = serde_json::to_string_pretty(&self.transactions)?;
        std::fs::write(path, json)?;

        if self.pretty_print {
            println!(
                "\x1b[36m[TX Debugger]\x1b[0m Exported {} transactions to {:?}",
                self.transactions.len(),
                path
            );
        }

        Ok(())
    }

    /// Get transaction count
    pub fn transaction_count(&self) -> usize {
        self.transactions.len()
    }

    /// Get confirmed transaction count
    pub fn confirmed_count(&self) -> usize {
        self.transactions
            .iter()
            .filter(|tx| tx.status == TransactionStatus::Confirmed)
            .count()
    }

    /// Print summary statistics
    pub fn print_summary(&self) {
        if !self.pretty_print {
            return;
        }

        let total = self.transactions.len();
        let confirmed = self.confirmed_count();
        let failed = self
            .transactions
            .iter()
            .filter(|tx| tx.status == TransactionStatus::Failed)
            .count();
        let pending = total - confirmed - failed;

        println!("\n\x1b[36m[TX Debugger]\x1b[0m Summary:");
        println!("  Total transactions: {}", total);
        println!("  \x1b[32mConfirmed: {}\x1b[0m", confirmed);
        println!("  \x1b[33mPending: {}\x1b[0m", pending);
        println!("  \x1b[31mFailed: {}\x1b[0m", failed);
    }
}

impl Default for TransactionDebugger {
    fn default() -> Self {
        Self::new(None, true, None)
    }
}

/// Bevy plugin for transaction debugging
pub struct TransactionDebuggerPlugin {
    pub log_file: Option<PathBuf>,
    pub pretty_print: bool,
    pub game_id: Option<u64>,
}

impl Default for TransactionDebuggerPlugin {
    fn default() -> Self {
        Self {
            log_file: None,
            pretty_print: true,
            game_id: None,
        }
    }
}

impl Plugin for TransactionDebuggerPlugin {
    fn build(&self, app: &mut App) {
        let log_file = self.log_file.as_ref().and_then(|path| {
            File::create(path)
                .map_err(|e| {
                    eprintln!("Failed to create log file: {}", e);
                    e
                })
                .ok()
        });

        let debugger = TransactionDebugger::new(log_file, self.pretty_print, self.game_id);

        app.insert_resource(debugger);
        // TODO: Migrate to Bevy 0.18 event system
        // .add_systems(Update, debug_rollup_events);
    }
}

// TODO: Migrate to Bevy 0.18 event system
// /// System that listens to rollup events and logs them
// fn debug_rollup_events(
//     mut debugger: ResMut<TransactionDebugger>,
//     mut rollup_events: EventReader<RollupEvent>,
// ) {
//     for event in rollup_events.read() {
//         debugger.log_rollup_event(event);
//     }
// }

/// Calculate a simple batch hash from moves
fn calculate_batch_hash(game_id: u64, moves: &[String], next_fens: &[String]) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    game_id.hash(&mut hasher);
    moves.hash(&mut hasher);
    next_fens.hash(&mut hasher);

    format!("{:x}", hasher.finish())
}

/// Get current Unix timestamp
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debugger_logs_event() {
        let mut debugger = TransactionDebugger::new(None, false, None);

        let tx = RollupTransaction {
            timestamp: 1708886400,
            game_id: 12345,
            tx_type: TransactionType::BatchProposed,
            status: TransactionStatus::Pending,
            batch_hash: "abc123".to_string(),
            moves: vec!["e2e4".to_string()],
            solana_signature: None,
            error: None,
            p2p_message: None,
        };

        debugger.log_event(tx);
        assert_eq!(debugger.transaction_count(), 1);
    }

    #[test]
    fn test_game_id_filter() {
        let mut debugger = TransactionDebugger::new(None, false, Some(12345));

        debugger.log_event(RollupTransaction {
            timestamp: 1708886400,
            game_id: 12345,
            tx_type: TransactionType::BatchProposed,
            status: TransactionStatus::Pending,
            batch_hash: "abc".to_string(),
            moves: vec![],
            solana_signature: None,
            error: None,
            p2p_message: None,
        });

        debugger.log_event(RollupTransaction {
            timestamp: 1708886400,
            game_id: 99999, // Different game
            tx_type: TransactionType::BatchProposed,
            status: TransactionStatus::Pending,
            batch_hash: "def".to_string(),
            moves: vec![],
            solana_signature: None,
            error: None,
            p2p_message: None,
        });

        assert_eq!(debugger.transaction_count(), 1);
    }
}
