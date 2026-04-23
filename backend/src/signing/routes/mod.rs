//! HTTP route handlers for the signing service.
//!
//! This module organizes all API endpoints into submodules:
//! - `main`: Core API (sessions, moves, games, auth)
//! - `identity`: KYC/identity registration
//! - `matchmaking`: ELO-based player matching
//! - `tournament`: Tournament bracket management
//! - `history`: Game history queries
//! - `dispute`: Dispute resolution

pub mod main;
pub mod auth;
pub mod identity;
pub mod matchmaking;
pub mod tournament;
pub mod pdf_mailer;
pub mod kyc;
pub mod history;
pub mod dispute;
pub mod relayer;
