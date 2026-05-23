//! HTTP route handlers for the signing service.
//!
//! This module organizes all API endpoints into submodules:
//! - `main`: Core API (sessions, moves, games, auth)
//! - `identity`: KYC/identity registration
//! - `matchmaking`: ELO-based player matching
//! - `tournament`: Tournament bracket management
//! - `rates`: Fiat/crypto exchange rates for wager tier pricing
//! - `history`: Game history queries
//! - `dispute`: Dispute resolution

pub mod anticheat;
pub mod chat;
pub mod main;
pub mod auth;
pub mod identity;
pub mod matchmaking;
pub mod tournament;
pub mod rates;
pub mod pdf_mailer;
pub mod kyc;
pub mod history;
pub mod dispute;
pub mod relayer;
pub mod debug;
pub mod archive;
pub mod admin;
pub mod global_session;
pub mod wallet;
pub mod external_elo;
pub mod lichess_oauth;
