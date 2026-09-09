use crate::client::retry::RetryConfig;
use crate::client::{BraidClient, Subscription};
use crate::types::{BraidRequest, Update, Version};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, warn};

#[derive(Clone, Default)]
pub struct ChannelStatus {
    online: Arc<AtomicBool>,
    outstanding_puts: Arc<AtomicUsize>,
}

impl ChannelStatus {
    #[must_use]
    pub fn is_online(&self) -> bool {
        self.online.load(Ordering::Relaxed)
    }

    #[must_use]
    pub fn outstanding_puts(&self) -> usize {
        self.outstanding_puts.load(Ordering::Relaxed)
    }

    fn set_online(&self, value: bool) {
        self.online.store(value, Ordering::Relaxed);
    }
}

#[derive(Debug, Clone)]
pub struct ReconnectPolicy {
    pub initial_backoff: Duration,
    pub max_backoff: Duration,
    pub max_attempts: Option<u32>,
}

impl Default for ReconnectPolicy {
    fn default() -> Self {
        Self {
            initial_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(30),
            max_attempts: None,
        }
    }
}

impl ReconnectPolicy {
    fn backoff_after(&self, attempt: u32) -> Duration {
        let factor = 2_u32.saturating_pow(attempt.min(16));
        self.initial_backoff
            .saturating_mul(factor)
            .min(self.max_backoff)
    }
}

pub struct ReliableChannel {
    client: BraidClient,
    url: String,
    subscription: Option<Subscription>,
    resume_from: Option<Version>,
    policy: ReconnectPolicy,
    retry: RetryConfig,
    status: ChannelStatus,
    attempts: u32,
}

impl ReliableChannel {
    #[must_use]
    pub fn new(client: BraidClient, url: impl Into<String>) -> Self {
        Self {
            client,
            url: url.into(),
            subscription: None,
            resume_from: None,
            policy: ReconnectPolicy::default(),
            retry: RetryConfig::default(),
            status: ChannelStatus::default(),
            attempts: 0,
        }
    }

    #[must_use]
    pub fn with_reconnect_policy(mut self, policy: ReconnectPolicy) -> Self {
        self.policy = policy;
        self
    }

    #[must_use]
    pub fn with_retry(mut self, retry: RetryConfig) -> Self {
        self.retry = retry;
        self
    }

    #[must_use]
    pub fn resuming_from(mut self, version: Version) -> Self {
        self.resume_from = Some(version);
        self
    }

    #[must_use]
    pub fn status(&self) -> ChannelStatus {
        self.status.clone()
    }

    #[must_use]
    pub fn version(&self) -> Option<&Version> {
        self.resume_from.as_ref()
    }

    pub async fn next(&mut self) -> Option<Update> {
        loop {
            if self.subscription.is_none() && !self.connect().await {
                return None;
            }

            let Some(sub) = self.subscription.as_mut() else {
                return None;
            };

            match sub.next().await {
                Some(Ok(update)) => {
                    self.attempts = 0;
                    if let Some(v) = update.primary_version() {
                        self.resume_from = Some(v.clone());
                    }
                    return Some(update);
                }
                Some(Err(e)) => {
                    warn!("[braid-channel] {} dropped: {e}", self.url);
                    self.drop_connection();
                }
                None => {
                    debug!("[braid-channel] {} closed by server", self.url);
                    self.drop_connection();
                }
            }
        }
    }

    pub async fn put(&self, body: impl Into<bytes::Bytes>) -> crate::Result<()> {
        let body = body.into();
        self.status.outstanding_puts.fetch_add(1, Ordering::Relaxed);

        let mut request = BraidRequest::new()
            .with_method("PUT")
            .with_body(body)
            .with_retry(self.retry.clone());
        if let Some(parent) = &self.resume_from {
            request = request.with_parent(parent.clone());
        }

        let result = self.client.fetch(&self.url, request).await;
        self.status.outstanding_puts.fetch_sub(1, Ordering::Relaxed);
        result.map(|_| ())
    }

    async fn connect(&mut self) -> bool {
        loop {
            if let Some(max) = self.policy.max_attempts {
                if self.attempts >= max {
                    warn!(
                        "[braid-channel] giving up on {} after {} attempts",
                        self.url, self.attempts
                    );
                    return false;
                }
            }

            let mut request = BraidRequest::new().subscribe();
            // Resuming means the server replays only what is newer. Omitting this
            // is always safe, just noisier — the caller sees history again.
            if let Some(parent) = &self.resume_from {
                request = request.with_parent(parent.clone());
            }

            match self.client.subscribe(&self.url, request).await {
                Ok(sub) => {
                    debug!("[braid-channel] connected to {}", self.url);
                    self.subscription = Some(sub);
                    self.status.set_online(true);
                    self.attempts = 0;
                    return true;
                }
                Err(e) => {
                    let wait = self.policy.backoff_after(self.attempts);
                    self.attempts += 1;
                    warn!(
                        "[braid-channel] connect to {} failed ({e}); retrying in {:?}",
                        self.url, wait
                    );
                    crate::client::utils::sleep(wait).await;
                }
            }
        }
    }

    fn drop_connection(&mut self) {
        self.subscription = None;
        self.status.set_online(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_doubles_then_saturates() {
        let policy = ReconnectPolicy {
            initial_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(30),
            max_attempts: None,
        };
        assert_eq!(policy.backoff_after(0), Duration::from_secs(1));
        assert_eq!(policy.backoff_after(1), Duration::from_secs(2));
        assert_eq!(policy.backoff_after(2), Duration::from_secs(4));
        assert_eq!(policy.backoff_after(30), Duration::from_secs(30));
    }

    #[test]
    fn status_starts_offline_with_nothing_pending() {
        let status = ChannelStatus::default();
        assert!(!status.is_online());
        assert_eq!(status.outstanding_puts(), 0);
    }

    #[test]
    fn status_clones_share_one_state() {
        let a = ChannelStatus::default();
        let b = a.clone();
        a.set_online(true);
        assert!(b.is_online(), "clones must observe the same channel");
    }
}
