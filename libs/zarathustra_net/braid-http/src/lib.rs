#![cfg_attr(test, deny(warnings))]
#![forbid(unsafe_code)]
#![cfg_attr(
    not(test),
    deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)
)]
pub mod client;
pub mod error;
pub mod traits;
pub mod types;

pub use client::BraidClient;
pub use client::{HeartbeatConfig, Subscription};
pub use error::{BraidError, Result};
pub use types::{BraidRequest, BraidResponse, Patch, Update, Version};
#[cfg(not(target_arch = "wasm32"))]
pub mod durable;
pub mod protocol;
pub mod server;
