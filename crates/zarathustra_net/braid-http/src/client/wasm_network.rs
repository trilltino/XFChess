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
