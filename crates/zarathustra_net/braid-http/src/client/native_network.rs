use crate::client::parser::MessageParser;
use crate::client::HeartbeatConfig;
use crate::error::{BraidError, Result};
use crate::protocol;
use crate::traits::{BraidNetwork, SubscriptionStreamHandle};
use crate::types::{BraidRequest, BraidResponse};
use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;

pub struct NativeNetwork {
    client: Client,
}

impl NativeNetwork {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    pub fn client(&self) -> &Client {
        &self.client
    }
}

#[async_trait]
impl BraidNetwork for NativeNetwork {
    async fn fetch(&self, url: &str, request: BraidRequest) -> Result<BraidResponse> {
        let method = match request.method.to_uppercase().as_str() {
            "POST" => reqwest::Method::POST,
            "PUT" => reqwest::Method::PUT,
            "DELETE" => reqwest::Method::DELETE,
            "PATCH" => reqwest::Method::PATCH,
            _ => reqwest::Method::GET,
        };

        let mut req_builder = self.client.request(method.clone(), url);

        for (k, v) in &request.extra_headers {
            req_builder = req_builder.header(k, v);
        }

        if !request.body.is_empty() {
            let ct = request
                .content_type
                .as_deref()
                .unwrap_or("application/json");
            req_builder = req_builder.header(reqwest::header::CONTENT_TYPE, ct);
            req_builder = req_builder.body(request.body.clone());
        }

        if let Some(versions) = &request.version {
            let header_val = if url.contains("braid.org") {
                protocol::format_version_header_json(versions)
            } else {
                protocol::format_version_header(versions)
            };
            req_builder = req_builder.header("Version", header_val);
        }
        if let Some(parents) = &request.parents {
            let header_val = if url.contains("braid.org") {
                protocol::format_version_header_json(parents)
            } else {
                protocol::format_version_header(parents)
            };
            req_builder = req_builder.header("Parents", header_val);
        }
        if request.subscribe {
            req_builder = req_builder.header("subscribe", "true");
        }
        if let Some(peer) = &request.peer {
            // Only add quotes if not already present
            let peer_val = if peer.starts_with('"') && peer.ends_with('"') {
                peer.clone()
            } else {
                format!("\"{}\"", peer)
            };
            req_builder = req_builder.header("Peer", peer_val);
        }
        if let Some(merge_type) = &request.merge_type {
            req_builder = req_builder.header("merge-type", merge_type);
        }

        tracing::debug!(
            "[BraidHTTP-Out] {} {} headers: {:?}",
            method,
            url,
            request.extra_headers
        );

        // Force a new connection for subscriptions by disabling connection reuse
        let response = req_builder
            .header("Connection", "close")
            .send()
            .await
            .map_err(|e| BraidError::Http(e.to_string()))?;

        let status = response.status().as_u16();
        let mut headers = std::collections::BTreeMap::new();
        for (k, v) in response.headers() {
            if let Ok(val) = v.to_str() {
                headers.insert(k.as_str().to_string(), val.to_string());
            }
        }

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

    async fn subscribe(
        &self,
        url: &str,
        mut request: BraidRequest,
    ) -> Result<SubscriptionStreamHandle> {
        request.subscribe = true;
        let mut req_builder = self.client.get(url).header("Subscribe", "true");

        for (k, v) in &request.extra_headers {
            req_builder = req_builder.header(k, v);
        }

        if let Some(versions) = &request.version {
            let header_val = if url.contains("braid.org") {
                protocol::format_version_header_json(versions)
            } else {
                protocol::format_version_header(versions)
            };
            req_builder = req_builder.header("Version", header_val);
        }

        if let Some(parents) = &request.parents {
            let header_val = if url.contains("braid.org") {
                protocol::format_version_header_json(parents)
            } else {
                protocol::format_version_header(parents)
            };
            req_builder = req_builder.header("Parents", header_val);
        }

        if let Some(peer) = &request.peer {
            req_builder = req_builder.header("Peer", format!("\"{}\"", peer));
        }

        if let Some(merge_type) = &request.merge_type {
            req_builder = req_builder.header("merge-type", merge_type);
        }

        tracing::debug!(
            "[BraidHTTP-Sub-Out] GET {} headers: Subscribe=true, merge-type={:?}, Peer={:?}, extra={:?}",
            url,
            request.merge_type,
            request.peer,
            request.extra_headers
        );

        // For subscriptions, disable timeout (or set very long) since we're waiting for heartbeats/updates
        // Heartbeats are every 30s, so we need timeout > 30s. Using 5 minutes for safety.
        let response = req_builder
            .timeout(std::time::Duration::from_secs(300))
            .send()
            .await
            .map_err(|e| BraidError::Http(e.to_string()))?;

        let status = response.status();
        tracing::debug!("[BraidHTTP-Sub] Response status: {}", status);

        let mut headers = std::collections::BTreeMap::new();
        for (k, v) in response.headers() {
            if let Ok(val) = v.to_str() {
                headers.insert(k.as_str().to_lowercase(), val.to_string());
            }
        }

        tracing::debug!(
            "[BraidRequest] Response headers (normalized): {:?}",
            headers
        );

        let heartbeat = headers
            .get("heartbeats")
            .and_then(|v| HeartbeatConfig::from_header(v));

        let (tx, rx) = async_channel::bounded(100);
        let mut stream = response.bytes_stream();

        tokio::spawn(async move {
            // A 209 body is always a sequence of self-describing update blocks,
            // whatever the response-level Content-Length or Transfer-Encoding say.
            let mut parser = MessageParser::for_subscription();
            tracing::debug!("[BraidHTTP-Parser] Started ({} response)", status);

            while let Some(chunk_res) = stream.next().await {
                match chunk_res {
                    Ok(chunk) => {
                        tracing::trace!(
                            "[BraidHTTP-Parser] Received chunk of {} bytes: {:?}",
                            chunk.len(),
                            chunk
                                .iter()
                                .take(50)
                                .map(|b| *b as char)
                                .collect::<String>()
                        );
                        match parser.feed(&chunk) {
                            Ok(messages) => {
                                // A heartbeat yields no messages at all, which is
                                // exactly why it must never reach a subscriber.
                                tracing::trace!(
                                    "[BraidHTTP-Parser] Parsed {} messages",
                                    messages.len()
                                );
                                for msg in messages {
                                    let update = crate::client::utils::message_to_update(msg);
                                    let _ = tx.send(Ok(update)).await;
                                }
                            }
                            Err(e) => {
                                tracing::error!("[BraidHTTP-Parser] Parse error: {}", e);
                                let _ = tx.send(Err(e)).await;
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("[BraidHTTP-Parser] Stream error: {}", e);
                        let _ = tx.send(Err(BraidError::Http(e.to_string()))).await;
                        break;
                    }
                }
            }
            tracing::debug!("[BraidHTTP-Parser] Stream ended");
        });

        Ok(SubscriptionStreamHandle {
            updates: rx,
            heartbeat,
        })
    }
}
