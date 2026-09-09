pub mod client;
pub mod error;
pub mod traits;
pub mod types;

pub mod protocol;
pub mod server;

pub use client::{BraidClient, HeartbeatConfig, ReliableChannel, Subscription};
pub use error::{BraidError, Result};
pub use types::{BraidRequest, BraidResponse, Patch, Update, Version};

pub mod prelude {
    pub use crate::client::{BraidClient, HeartbeatConfig, ReliableChannel, Subscription};
    pub use crate::error::{BraidError, Result};
    pub use crate::protocol::formatter::{format_heartbeat, format_update};
    pub use crate::server::SubscriptionResponse;
    pub use crate::types::{BraidRequest, BraidResponse, Patch, Update, Version};
}
