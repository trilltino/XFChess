//! A pure Braid-compatible sidecar for Stockfish AI.
//!
//! This module runs Stockfish on a dedicated thread, receiving board positions
//! (FEN strings) via a channel and yielding UCI best-move strings back.
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────┐  FEN (String)  ┌─────────────────────┐  UCI move  ┌──────────┐
//! │ Braid / Bevy │ ──────────────►│ BraidStockfishSidecar│ ─────────►│ Braid /  │
//! │  main thread  │               │  (dedicated thread)  │           │ Bevy ECS │
//! └──────────────┘                └─────────────────────┘           └──────────┘
//! ```

use std::error::Error;
use std::sync::mpsc;
use stockfish::Stockfish;

/// Default search depth when none is specified.
const DEFAULT_DEPTH: u32 = 12;

/// A channel-driven wrapper around the Stockfish UCI engine.
///
/// Create with [`BraidStockfishSidecar::with_channels`], then call [`run`]
/// on a dedicated thread. The sidecar blocks on the `fen_rx` channel,
/// computes the best move, and sends the UCI string back on `move_tx`.
pub struct BraidStockfishSidecar {
    engine: Stockfish,
    depth: u32,
    /// Receives FEN strings from the game.
    fen_rx: mpsc::Receiver<String>,
    /// Sends UCI move strings back to the game.
    move_tx: tokio::sync::mpsc::Sender<String>,
}

impl BraidStockfishSidecar {
    /// Create a sidecar wired to the given channels.
    ///
    /// * `fen_rx` — blocking receiver for FEN position strings.
    /// * `move_tx` — async sender for UCI best-move strings (e.g. `"e2e4"`).
    ///
    /// The Stockfish binary must be available in `PATH` as `stockfish`
    /// (or `stockfish.exe` on Windows).
    pub fn with_channels(
        fen_rx: mpsc::Receiver<String>,
        move_tx: tokio::sync::mpsc::Sender<String>,
    ) -> Result<Self, Box<dyn Error>> {
        // Look for bundled binary first, then fall back to PATH
        let bundled = if cfg!(target_os = "windows") {
            "assets/bin/stockfish.exe"
        } else {
            "assets/bin/stockfish"
        };

        let path = if std::path::Path::new(bundled).exists() {
            log::info!("[Stockfish Sidecar] Using bundled binary: {bundled}");
            bundled
        } else {
            let fallback = if cfg!(target_os = "windows") {
                "stockfish.exe"
            } else {
                "stockfish"
            };
            log::info!("[Stockfish Sidecar] Bundled binary not found, trying PATH: {fallback}");
            fallback
        };
        let mut engine = Stockfish::new(path)?;
        engine.setup_for_new_game()?;
        Ok(Self {
            engine,
            depth: DEFAULT_DEPTH,
            fen_rx,
            move_tx,
        })
    }

    /// Set the search depth used for subsequent moves.
    pub fn set_depth(&mut self, depth: u32) {
        self.depth = depth;
    }

    /// Blocking event loop — call this on a **dedicated `std::thread`**.
    ///
    /// The loop runs until the `fen_rx` channel is closed (all senders
    /// dropped), at which point it sends "quit" to Stockfish and returns.
    pub fn run(&mut self) {
        log::info!(
            "[Stockfish Sidecar] run-loop started (depth={})",
            self.depth
        );

        while let Ok(fen) = self.fen_rx.recv() {
            log::info!("[Stockfish Sidecar] Received FEN: {}", fen);

            // Set position and depth, then search
            if let Err(e) = self.engine.set_fen_position(&fen) {
                log::error!("[Stockfish Sidecar] set_fen_position failed: {e}");
                continue;
            }
            self.engine.set_depth(self.depth);

            let output = match self.engine.go() {
                Ok(o) => o,
                Err(e) => {
                    log::error!("[Stockfish Sidecar] go() failed: {e}");
                    continue;
                }
            };

            let best = output.best_move().clone();
            log::info!("[Stockfish Sidecar] Best move: {best}");

            // Send the move back (blocking_send is fine — we are on a
            // dedicated thread and the receiver is polled by Bevy).
            if self.move_tx.blocking_send(best).is_err() {
                log::warn!("[Stockfish Sidecar] move_tx closed, stopping run loop");
                break;
            }
        }

        log::info!("[Stockfish Sidecar] run-loop exited, shutting down engine");
        let _ = self.engine.quit();
    }

    /// One-shot convenience: evaluate a single FEN and return the best move.
    ///
    /// This does **not** use the channel loop — it is useful for tests or
    /// ad-hoc queries outside the normal Braid flow.
    pub fn get_best_move_oneshot(
        engine: &mut Stockfish,
        fen: &str,
        depth: u32,
    ) -> Result<String, Box<dyn Error>> {
        engine.set_fen_position(fen)?;
        engine.set_depth(depth);
        let output = engine.go()?;
        Ok(output.best_move().clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that the default depth constant is sensible.
    #[test]
    fn default_depth_is_reasonable() {
        assert!(DEFAULT_DEPTH >= 1 && DEFAULT_DEPTH <= 30);
    }

    /// Verify channel wiring compiles and types line up.
    /// (Does NOT require a real Stockfish binary.)
    #[test]
    fn channel_types_compile() {
        let (_fen_tx, fen_rx) = mpsc::channel::<String>();
        let (move_tx, _move_rx) = tokio::sync::mpsc::channel::<String>(10);

        // We only check that the types are accepted; construction
        // would fail without a real binary, so we don't call it here.
        let _fn_ptr: fn(
            mpsc::Receiver<String>,
            tokio::sync::mpsc::Sender<String>,
        ) -> Result<BraidStockfishSidecar, Box<dyn Error>> = BraidStockfishSidecar::with_channels;

        // Ensure the channels are the right types
        drop(fen_rx);
        drop(move_tx);
    }
}
