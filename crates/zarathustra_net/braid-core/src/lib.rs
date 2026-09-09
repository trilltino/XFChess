pub mod core;

// Top-level re-exports for common usage
pub use crate::core::error::{BraidError, Result};
pub use crate::core::{BraidRequest, BraidResponse, Patch, Update, Version};
pub use braid_http::types;

#[cfg(feature = "client")]
pub use crate::core::{BraidClient, ClientConfig, Subscription};
#[cfg(feature = "client")]
pub use braid_http::client;
