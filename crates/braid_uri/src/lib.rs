//! # braid_uri
//!
//! Full Braid-HTTP chess game messaging for XFChess.
//!
//! This crate provides everything needed to publish and subscribe to chess
//! game events over the Braid-HTTP protocol.
//!
//! ## Core types
//!
//! | Type | Description |
//! |---|---|
//! | [`ChessUri`] | Typed resource URI (`/game/{id}/moves`) |
//! | [`ChessMessage`] | All game events (move, resign, draw, clock, engine) |
//! | [`BraidPatch`] | Low-level Braid version patch (version hash + body) |
//! | [`ChessPublisher`] | Publishes moves/events via Braid-HTTP PUT |
//! | [`ChessSubscriber`] | Subscribes to Braid-HTTP SSE update stream |
//!
//! ## Quick start
//!
//! ```rust,no_run
//! use braid_uri::{ChessPublisher, ChessSubscriber, MovePayload};
//!
//! #[tokio::main]
//! async fn main() {
//!     // Publisher (game server / local player)
//!     let mut pub_ = ChessPublisher::new("http://localhost:3000", "ABCD42").unwrap();
//!     let payload = MovePayload::from_uci("e2e4", "new_fen", 1, "alice");
//!     pub_.publish_move(&payload).await.unwrap();
//!
//!     // Subscriber (opponent)
//!     let sub = ChessSubscriber::new("http://localhost:3000", "ABCD42").unwrap();
//!     let (rx, _task) = sub.subscribe_moves().await.unwrap();
//!     while let Ok(msg) = rx.recv().await {
//!         println!("{:?}", msg);
//!     }
//! }
//! ```

pub mod error;
pub mod message;
pub mod patch;
pub mod publisher;
pub mod subscriber;
pub mod uri;

pub use error::BraidUriError;
pub use message::{ChessMessage, ClockState, EngineHint, MovePayload};
pub use patch::{version_hash, BraidPatch};
pub use publisher::ChessPublisher;
pub use subscriber::ChessSubscriber;
pub use uri::{ChessPath, ChessUri};
