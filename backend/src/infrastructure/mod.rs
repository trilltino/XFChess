//! Infrastructure layer for the XFChess backend.
//!
//! This module provides low-level infrastructure components:
//! - Database initialization and migrations
//! - Router building and merging
//! - Background task spawning
//! - Ngrok integration for development
//! - Authentication middleware

pub mod auth_middleware;
pub mod database;
pub mod ngrok;
pub mod router;
pub mod tasks;

pub use auth_middleware::{require_api_key, require_relay_or_jwt, require_relay_secret};
pub use database::{initialize_pools, run_migrations};
pub use ngrok::{get_ngrok_url, start_ngrok_tunnel};
pub use router::build_app_router;
pub use tasks::spawn_background_tasks;
