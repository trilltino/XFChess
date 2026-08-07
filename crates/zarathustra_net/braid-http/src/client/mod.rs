//! Braid HTTP client implementation.

mod channel;
mod config;
mod fetch;
#[cfg(test)]
mod fuzzer;
mod headers;
mod multiplex;
#[cfg(not(target_arch = "wasm32"))]
pub mod native_network;
mod parser;
pub mod retry;
mod subscription;
pub(crate) mod utils;
#[cfg(target_arch = "wasm32")]
pub mod wasm_network;

pub use channel::{ChannelStatus, ReconnectPolicy, ReliableChannel};
pub use config::ClientConfig;
pub use fetch::BraidClient;
pub use headers::{BraidHeaders, HeaderParser};
pub use parser::{parse_status_line, Message, MessageParser, ParseState};
pub use retry::{parse_retry_after, RetryConfig, RetryDecision, RetryState};
pub use subscription::{HeartbeatConfig, Subscription};
pub use utils::{format_content_range, parse_content_range, parse_heartbeat};
