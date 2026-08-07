//! Browser transport — a placeholder.
//!
//! Nothing in this workspace targets wasm today; the game client is native and
//! the web frontend uses the JavaScript `braid-http` library directly. The module
//! exists so the `wasm` feature keeps compiling, and so the shape of the work is
//! obvious if a browser client is ever wanted: implement these two methods over
//! `fetch` + `ReadableStream`, feeding bytes to
//! [`MessageParser::for_subscription`](crate::client::MessageParser::for_subscription)
//! exactly as [`native_network`](super::native_network) does.

use crate::error::{BraidError, Result};
use crate::traits::{BraidNetwork, SubscriptionStreamHandle};
use crate::types::{BraidRequest, BraidResponse};
use async_trait::async_trait;

pub struct WasmNetwork;

#[async_trait]
impl BraidNetwork for WasmNetwork {
    async fn fetch(&self, _url: &str, _request: BraidRequest) -> Result<BraidResponse> {
        Err(BraidError::Internal(
            "WasmNetwork::fetch is not implemented".to_string(),
        ))
    }

    async fn subscribe(
        &self,
        _url: &str,
        _request: BraidRequest,
    ) -> Result<SubscriptionStreamHandle> {
        Err(BraidError::Internal(
            "WasmNetwork::subscribe is not implemented".to_string(),
        ))
    }
}
