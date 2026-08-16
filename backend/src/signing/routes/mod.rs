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

/// Player moderation, anti-cheat review, treasury/wallet ops. See `admin.rs` module docs.
pub mod admin;
pub mod anticheat;
pub mod archive;
pub mod auth;
pub mod casual_games;
pub mod debug;
pub mod dispute;
pub mod external_elo;
pub mod game_log;
pub mod global_session;
pub mod history;
pub mod identity;
pub mod kyc;
pub mod lichess_oauth;
pub mod mailer;
pub mod main;
pub mod matchmaking;
pub mod puzzle;
pub mod rates;
pub mod rpc_proxy;
pub mod tournament;
pub mod wallet;
