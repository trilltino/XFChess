//! HTTP route handlers for the signing service.
//!
//! This module organizes all API endpoints into submodules:
//! - `main`: Core API (sessions, moves, games, auth)
//! - `identity`: KYC/identity registration
//! - `matchmaking`: ELO-based player matching
//! - `tournament`: Tournament bracket management

pub mod auth;
pub mod identity;
pub mod main;
pub mod matchmaking;
pub mod tournament;
pub mod pdf_mailer;

use axum::Router;

use crate::signing::AppState;

/// Creates the main API routes router.
///
/// Combines all route modules into the main application router.
pub fn create_routes() -> Router<AppState> {
    main::routes()
}
