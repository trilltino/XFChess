use crate::error::{BraidError, Result};
use crate::types::Update;
use futures::Stream;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeartbeatConfig {
    pub interval: Duration,
    pub timeout: Duration,
}

impl HeartbeatConfig {
    #[must_use]
    pub fn new(interval: Duration) -> Self {
        Self {
            interval,
            timeout: interval.mul_f64(1.2) + Duration::from_secs(3),
        }
    }

    #[must_use]
    pub fn from_secs(secs: f64) -> Self {
        Self::new(Duration::from_secs_f64(secs))
    }

    #[must_use]
    pub fn from_header(value: &str) -> Option<Self> {
        let v = value.trim();
        let num = v.strip_suffix('s').unwrap_or(v);
        num.parse::<f64>()
            .ok()
            .filter(|n| *n > 0.0 && n.is_finite())
            .map(Self::from_secs)
    }
}

pub struct Subscription {
    updates: Pin<Box<async_channel::Receiver<Result<Update>>>>,
    heartbeat: Option<HeartbeatConfig>,
    #[cfg(not(target_arch = "wasm32"))]
    deadline: Option<Pin<Box<tokio::time::Sleep>>>,
}

impl Subscription {
    #[must_use]
    pub fn new(updates: async_channel::Receiver<Result<Update>>) -> Self {
        Self {
            updates: Box::pin(updates),
            heartbeat: None,
            #[cfg(not(target_arch = "wasm32"))]
            deadline: None,
        }
    }

    #[must_use]
    pub fn with_heartbeat(
        updates: async_channel::Receiver<Result<Update>>,
        config: HeartbeatConfig,
    ) -> Self {
        Self::new(updates).heartbeat(Some(config))
    }

    #[must_use]
    pub fn heartbeat(mut self, config: Option<HeartbeatConfig>) -> Self {
        self.heartbeat = config;
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.deadline = config.map(|c| Box::pin(tokio::time::sleep(c.timeout)));
        }
        self
    }

    #[must_use]
    pub fn heartbeat_config(&self) -> Option<HeartbeatConfig> {
        self.heartbeat
    }

    pub async fn next(&mut self) -> Option<Result<Update>> {
        futures::StreamExt::next(self).await
    }

    fn touch(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        if let (Some(config), Some(deadline)) = (self.heartbeat, self.deadline.as_mut()) {
            deadline
                .as_mut()
                .reset(tokio::time::Instant::now() + config.timeout);
        }
    }
}

impl Stream for Subscription {
    type Item = Result<Update>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        // `async_channel::Receiver` is itself a Stream, so it is polled directly —
        // there is no self-referential future here to construct and pin per call.
        match this.updates.as_mut().poll_next(cx) {
            Poll::Ready(Some(item)) => {
                this.touch();
                return Poll::Ready(Some(item));
            }
            Poll::Ready(None) => return Poll::Ready(None),
            Poll::Pending => {}
        }

        #[cfg(not(target_arch = "wasm32"))]
        if let Some(deadline) = this.deadline.as_mut() {
            if std::future::Future::poll(deadline.as_mut(), cx).is_ready() {
                // Deliberately not re-armed: the subscription is over, and a
                // caller that keeps polling should keep seeing the timeout rather
                // than silently beginning a fresh wait.
                return Poll::Ready(Some(Err(BraidError::Timeout)));
            }
        }

        Poll::Pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Version;

    #[test]
    fn timeout_follows_the_reference_formula() {
        // 1.2 * 20 + 3
        assert_eq!(
            HeartbeatConfig::from_secs(20.0).timeout,
            Duration::from_secs(27)
        );
    }

    #[test]
    fn heartbeats_header_parses_both_spellings() {
        assert_eq!(
            HeartbeatConfig::from_header("20").unwrap().interval,
            Duration::from_secs(20)
        );
        assert_eq!(
            HeartbeatConfig::from_header("20s").unwrap().interval,
            Duration::from_secs(20)
        );
        assert!(HeartbeatConfig::from_header("nonsense").is_none());
        assert!(HeartbeatConfig::from_header("0").is_none());
    }

    #[tokio::test]
    async fn updates_are_delivered_in_order_then_the_stream_ends() {
        let (tx, rx) = async_channel::unbounded();
        tx.send(Ok(Update::snapshot(Version::new("1"), "a")))
            .await
            .unwrap();
        tx.send(Ok(Update::snapshot(Version::new("2"), "b")))
            .await
            .unwrap();
        drop(tx);

        let mut sub = Subscription::new(rx);
        assert_eq!(sub.next().await.unwrap().unwrap().body_str(), Some("a"));
        assert_eq!(sub.next().await.unwrap().unwrap().body_str(), Some("b"));
        assert!(sub.next().await.is_none());
    }

    #[tokio::test(start_paused = true)]
    async fn a_silent_server_times_out() {
        // `_tx` is held so the channel never closes — only the deadline can fire.
        let (_tx, rx) = async_channel::unbounded::<Result<Update>>();
        let mut sub = Subscription::with_heartbeat(rx, HeartbeatConfig::from_secs(20.0));

        assert!(matches!(sub.next().await, Some(Err(BraidError::Timeout))));
    }

    #[tokio::test(start_paused = true)]
    async fn a_quiet_but_live_stream_never_times_out() {
        let (tx, rx) = async_channel::unbounded();
        let mut sub = Subscription::with_heartbeat(rx, HeartbeatConfig::from_secs(20.0));

        for i in 0..5 {
            tokio::time::sleep(Duration::from_secs(20)).await;
            tx.send(Ok(Update::snapshot(Version::new(i.to_string()), "x")))
                .await
                .unwrap();
            assert!(
                sub.next().await.is_some_and(|r| r.is_ok()),
                "a stream with regular traffic must not time out (step {i})"
            );
        }
    }

    #[tokio::test]
    async fn without_a_heartbeat_config_there_is_no_deadline() {
        let (_tx, rx) = async_channel::unbounded::<Result<Update>>();
        let mut sub = Subscription::new(rx);
        assert!(sub.heartbeat_config().is_none());
        assert!(
            tokio::time::timeout(Duration::from_millis(50), sub.next())
                .await
                .is_err(),
            "should still be waiting, not erroring"
        );
    }
}
