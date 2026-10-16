#[cfg(target_arch = "wasm32")]
compile_error!(
    "braid-http supports native clients; browser clients need a browser transport implementation"
);

pub mod client;
pub mod error;
pub mod types;

pub mod protocol;
pub mod server;

pub use client::{BraidClient, HeartbeatConfig, Subscription};
pub use error::{BraidError, Result};
pub use types::{BraidRequest, BraidResponse, Patch, Update, Version};

pub mod prelude {
    pub use crate::client::{BraidClient, HeartbeatConfig, Subscription};
    pub use crate::error::{BraidError, Result};
    pub use crate::protocol::formatter::{format_heartbeat, format_update};
    pub use crate::server::SubscriptionResponse;
    pub use crate::types::{BraidRequest, BraidResponse, Patch, Update, Version};
}
