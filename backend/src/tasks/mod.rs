//! Background tasks for the XFChess backend.
//!
//! This module provides long-running background services:
//! - Matchmaking: ELO-based player pairing
//! - Tournament Scheduler: auto-start scheduled tournaments
//! - Prize Distributor: auto-pays tournament winners once completed
//! - Settlement Worker: auto-finalizes finished games and pays wager escrows
//!
//! All of the above are wired up from [`crate::infrastructure::tasks::spawn_background_tasks`]
//! and [`crate::server::run`] — nothing in this module spawns itself; see those
//! two call sites for the actual startup wiring instead of grepping per-file.
//!
//! Two subsystems are deliberately *not* wired up yet, both incomplete rather
//! than just unused:
//! - [`crate::signing::swiss::spawn_orchestrator`] runs a pure event-loop
//!   (`while let Some(event) = event_rx.recv().await`) with no producer
//!   anywhere in the codebase that ever sends it an `OrchestratorEvent` —
//!   spawning it would be a permanently-idle task, not working automation.
//! - [`crate::signing::ws_subscriber::WebSocketSubscriber`] isn't read by
//!   any other code and the `ws_subscriber`/`polling` Cargo features that
//!   were meant to gate its use don't guard any `#[cfg(feature = ...)]` code
//!   anywhere — there's nothing for it to plug into yet.
//! Wire these in properly (find/build the producer side) rather than
//! resurrecting a bare `tokio::spawn` for either.

pub mod anticheat_worker;
pub mod archiver;
pub mod er_watch;
pub mod matchmaking;
pub mod queue;
pub mod settlement_worker;
pub mod tournament_scheduler;
