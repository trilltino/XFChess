use crate::client::stream::{self, SubscriptionFailure};
use crate::client::subscription::SubscriptionStreamHandle;
use crate::error::{BraidError, Result};
use crate::protocol;
use crate::types::{BraidRequest, BraidResponse};
use reqwest::Client;
use std::time::Duration;

#[derive(Clone)]
pub(crate) struct NativeNetwork {
    client: Client,
    request_timeout: Duration,
}

impl NativeNetwork {
    pub(crate) fn new(client: Client, request_timeout: Duration) -> Self {
        Self {
            client,
            request_timeout,
        }
    }

    pub(crate) fn client(&self) -> &Client {
        &self.client
    }

    fn request(&self, url: &str, request: BraidRequest) -> Result<reqwest::RequestBuilder> {
        let method = reqwest::Method::from_bytes(request.method.to_ascii_uppercase().as_bytes())
            .map_err(|e| BraidError::Config(e.to_string()))?;
        let mut builder = self.client.request(method, url);
        for (key, value) in request.extra_headers {
            builder = builder.header(key, value);
        }
        if let Some(value) = request.content_type {
            builder = builder.header("Content-Type", value);
        }
        if let Some(value) = request.version {
            builder = builder.header("Version", protocol::format_version_header(&value));
        }
        if let Some(value) = request.parents {
            builder = builder.header("Parents", protocol::format_version_header(&value));
        }
        if let Some(value) = request.ack {
            builder = builder.header("Ack", protocol::format_version_header(&value));
        }
        if request.subscribe {
            builder = builder.header("Subscribe", "true");
        }
        if let Some(value) = request.peer {
            builder = builder.header("Peer", format!("\"{}\"", value.trim_matches('"')));
        }
        if let Some(value) = request.merge_type {
            builder = builder.header("Merge-Type", value);
        }
        if let Some(value) = request.heartbeat_interval {
            builder = builder.header("Heartbeats", value.to_string());
        }
        if let Some(patches) = request.patches {
            builder = builder.header("Patches", patches.len());
            let mut wire = bytes::BytesMut::new();
            for patch in patches {
                protocol::formatter::format_patch(&mut wire, &patch)?;
            }
            builder = builder.body(wire.freeze());
        } else {
            builder = builder.body(request.body);
        }
        Ok(builder)
    }
}

impl NativeNetwork {
    pub(crate) async fn fetch(&self, url: &str, request: BraidRequest) -> Result<BraidResponse> {
        let response = self
            .request(url, request)?
            .timeout(self.request_timeout)
            .send()
            .await
            .map_err(|e| BraidError::Http(e.to_string()))?;
        let status = response.status().as_u16();
        let headers = response
            .headers()
            .iter()
            .filter_map(|(k, v)| v.to_str().ok().map(|v| (k.to_string(), v.to_owned())))
            .collect();
        let body = response
            .bytes()
            .await
            .map_err(|e| BraidError::Http(e.to_string()))?;
        Ok(BraidResponse {
            status,
            headers,
            body,
            is_subscription: status == 209,
        })
    }

    pub(crate) async fn open_subscription(
        &self,
        url: &str,
        request: &BraidRequest,
    ) -> std::result::Result<reqwest::Response, SubscriptionFailure> {
        let mut request = request.clone();
        request.subscribe = true;
        request.method = "GET".into();
        let request = self
            .request(url, request)?
            .build()
            .map_err(|e| BraidError::Config(e.to_string()))?;
        // Limit the handshake, not the lifetime of an active subscription.
        let response = tokio::time::timeout(self.request_timeout, self.client.execute(request))
            .await
            .map_err(|_| BraidError::Timeout)?
            .map_err(|e| BraidError::Http(e.to_string()))?;
        if response.status().as_u16() != 209 {
            return Err(SubscriptionFailure {
                error: BraidError::InvalidSubscriptionStatus(response.status().as_u16()),
                retry_after: response
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(crate::client::retry::parse_retry_after),
            });
        }
        Ok(response)
    }

    pub(crate) async fn subscribe(
        &self,
        url: &str,
        request: BraidRequest,
    ) -> Result<SubscriptionStreamHandle> {
        stream::subscribe(self.clone(), url, request).await
    }
}
