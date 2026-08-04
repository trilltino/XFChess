//! SQLite persistence: game/move archive, PGN compression, and resumable
//! player sessions. See the module README for the full schema layout.

pub mod repository;
pub mod schema;
pub mod sessions;

pub use repository::*;
pub use schema::*;
pub use sessions::*;
