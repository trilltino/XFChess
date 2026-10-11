use crate::client::config::ClientConfig;
use crate::client::native_network::NativeNetwork;
use crate::error::{BraidError, Result};
use crate::types::{BraidRequest, BraidResponse};
use std::sync::Arc;

#[derive(Clone)]
pub struct BraidClient {
    network: Arc<NativeNetwork>,
    config: Arc<ClientConfig>,
}

impl BraidClient {
    pub fn client(&self) -> &reqwest::Client {
        self.network.client()
    }

    pub fn new() -> Result<Self> {
        Self::with_config(ClientConfig::default())
    }

    pub fn with_config(config: ClientConfig) -> Result<Self> {
        {
            let mut builder = reqwest::Client::builder()
                .connect_timeout(std::time::Duration::from_secs(
                    config.connection_timeout_secs,
                ))
                .pool_idle_timeout(std::time::Duration::from_secs(90))
                .pool_max_idle_per_host(config.max_idle_connections_per_host);

            if !config.proxy_url.is_empty() {
                builder = builder.proxy(
                    reqwest::Proxy::all(&config.proxy_url)
                        .map_err(|e| BraidError::Config(e.to_string()))?,
                );
            }

            let client = builder
                .user_agent(concat!("braid-http/", env!("CARGO_PKG_VERSION")))
                .build()
                .map_err(|e| BraidError::Config(e.to_string()))?;
            let network = Arc::new(NativeNetwork::new(
                client,
                std::time::Duration::from_millis(config.request_timeout_ms),
            ));

            Ok(BraidClient {
                network,
                config: Arc::new(config),
            })
        }
    }

    /// The supplied client must have no total timeout for long-lived subscriptions.
    pub fn with_client(client: reqwest::Client) -> Result<Self> {
        let config = ClientConfig::default();
        Ok(BraidClient {
            network: Arc::new(NativeNetwork::new(
                client,
                std::time::Duration::from_millis(config.request_timeout_ms),
            )),
            config: Arc::new(ClientConfig::default()),
        })
    }

    pub async fn get(&self, url: &str) -> Result<BraidResponse> {
        self.fetch(url, BraidRequest::new()).await
    }

    pub async fn put(
        &self,
        url: &str,
        body: &str,
        mut request: BraidRequest,
    ) -> Result<BraidResponse> {
        request = request.with_method("PUT").with_body(body.to_string());

        if request.content_type.is_none() {
            request = request.with_content_type("application/json");
        }

        if request.version.is_none() {
            let random_version = uuid::Uuid::new_v4().to_string();
            request.version = Some(vec![crate::types::Version::new(&random_version)]);
        }

        self.fetch(url, request).await
    }

    pub async fn post(
        &self,
        url: &str,
        body: &str,
        mut request: BraidRequest,
    ) -> Result<BraidResponse> {
        request = request.with_method("POST").with_body(body.to_string());
        self.fetch(url, request).await
    }

    pub async fn fetch(&self, url: &str, request: BraidRequest) -> Result<BraidResponse> {
        self.fetch_with_retries(url, request).await
    }

    pub async fn subscribe(
        &self,
        url: &str,
        request: BraidRequest,
    ) -> Result<crate::client::Subscription> {
        let handle = self.network.subscribe(url, request).await?;
        Ok(crate::client::Subscription::from_transport(handle))
    }

    async fn fetch_with_retries(&self, url: &str, request: BraidRequest) -> Result<BraidResponse> {
        let retry_config = request.retry.clone().unwrap_or_else(|| {
            if self.config.max_retries == 0 {
                crate::client::retry::RetryConfig::no_retry()
            } else {
                crate::client::retry::RetryConfig::default()
                    .with_max_retries(self.config.max_retries)
                    .with_initial_backoff(std::time::Duration::from_millis(
                        self.config.retry_delay_ms,
                    ))
            }
        });

        let mut retry_state = crate::client::retry::RetryState::new(retry_config);

        loop {
            match self.fetch_internal(url, &request).await {
                Ok(response) => {
                    let status = response.status;
                    if (400..600).contains(&status) {
                        let retry_after = response
                            .headers
                            .get("retry-after")
                            .and_then(|v| crate::client::retry::parse_retry_after(v));

                        match retry_state.should_retry_status(status, retry_after) {
                            crate::client::retry::RetryDecision::Retry(delay) => {
                                if self.config.enable_logging {
                                    tracing::warn!(
                                        "Request status {} (attempt {}), retrying in {:?}",
                                        status,
                                        retry_state.attempts,
                                        delay
                                    );
                                }
                                crate::client::utils::sleep(delay).await;
                                continue;
                            }
                            crate::client::retry::RetryDecision::DontRetry => {
                                return Ok(response);
                            }
                        }
                    }
                    retry_state.reset();
                    return Ok(response);
                }
                Err(e) => {
                    if !matches!(
                        e,
                        BraidError::Http(_) | BraidError::Io(_) | BraidError::Timeout
                    ) {
                        return Err(e);
                    }
                    let is_abort = matches!(&e, BraidError::Aborted);

                    match retry_state.should_retry_error(is_abort) {
                        crate::client::retry::RetryDecision::Retry(delay) => {
                            if self.config.enable_logging {
                                tracing::warn!(
                                    "Request failed (attempt {}), retrying in {:?}: {}",
                                    retry_state.attempts,
                                    delay,
                                    e
                                );
                            }
                            crate::client::utils::sleep(delay).await;
                            continue;
                        }
                        crate::client::retry::RetryDecision::DontRetry => {
                            return Err(e);
                        }
                    }
                }
            }
        }
    }

    async fn fetch_internal(&self, url: &str, request: &BraidRequest) -> Result<BraidResponse> {
        self.network.fetch(url, request.clone()).await
    }

    pub fn config(&self) -> &ClientConfig {
        &self.config
    }
}
