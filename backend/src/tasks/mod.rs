//! Background tasks for the XFChess backend.
//!
//! This module provides long-running background services:
//! - Matchmaking: ELO-based player pairing
//! - Fee Claimer: Platform fee collection from vault

pub mod matchmaking;
pub mod fee_claimer;
pub mod tournament_scheduler;
