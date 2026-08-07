//! Server-side helpers for answering a Braid subscription.
//!
//! This module is deliberately transport-neutral: it produces header names and
//! values, and [`protocol::formatter`](crate::protocol::formatter) produces body
//! bytes. Nothing here depends on `axum`, `hyper`, or any particular server
//! framework, so the same helpers serve `xfchess-braid-server`, the backend game
//! log, and anything added later.
//!
//! # Answering a subscription
//!
//! ```no_run
//! use braid_http::server::{self, SubscriptionResponse};
//! use braid_http::protocol::formatter::{format_update, format_heartbeat};
//! use braid_http::types::{Update, Version};
//!
//! let res = SubscriptionResponse::new();
//! // res.status is 209; res.headers() go on the response
//! for (name, value) in res.headers() {
//!     // response.header(name, value)
//! }
//!
//! // then stream update bytes, and a heartbeat every `res.heartbeat_interval`
//! let bytes = format_update(&Update::snapshot(Version::new("1"), "{}")).unwrap();
//! let beat = format_heartbeat();
//! ```

use crate::protocol::constants::status;
use std::time::Duration;

/// The media type of a `209` body: a stream of HTTP-shaped update blocks.
///
/// Not `multipart/*`. Setting an explicit content type also stops Firefox from
/// content-sniffing a never-ending stream and hanging.
pub const HTTP_HISTORY: &str = "application/http-history";

/// Default seconds between heartbeats on an idle subscription.
///
/// Clients derive their liveness deadline from this (see
/// [`HeartbeatConfig`](crate::client::HeartbeatConfig)), so a server that sends
/// nothing for materially longer than this will be treated as dead.
pub const DEFAULT_HEARTBEAT_SECS: u64 = 20;

/// The status line and headers for a `209 Subscription` response.
#[derive(Debug, Clone)]
pub struct SubscriptionResponse {
    /// Always [`status::SUBSCRIPTION`] (209).
    pub status: u16,
    /// How often the server promises to send a heartbeat when idle.
    pub heartbeat_interval: Duration,
}

impl SubscriptionResponse {
    /// A subscription response using [`DEFAULT_HEARTBEAT_SECS`].
    #[must_use]
    pub fn new() -> Self {
        Self::with_heartbeat_secs(DEFAULT_HEARTBEAT_SECS)
    }

    /// A subscription response promising a heartbeat every `secs`.
    #[must_use]
    pub fn with_heartbeat_secs(secs: u64) -> Self {
        Self {
            status: status::SUBSCRIPTION,
            heartbeat_interval: Duration::from_secs(secs),
        }
    }

    /// The headers to set on the response, in `(name, value)` form.
    ///
    /// `Heartbeats` is what lets a client enable liveness detection without being
    /// configured out of band — omit it and the client cannot tell a quiet
    /// subscription from a dead one.
    #[must_use]
    pub fn headers(&self) -> Vec<(&'static str, String)> {
        vec![
            ("Content-Type", HTTP_HISTORY.to_string()),
            ("Cache-Control", "no-store".to_string()),
            ("Heartbeats", self.heartbeat_interval.as_secs().to_string()),
        ]
    }
}

impl Default for SubscriptionResponse {
    fn default() -> Self {
        Self::new()
    }
}

/// Whether a request is asking to subscribe rather than to read once.
///
/// Accepts every spelling in circulation: `Subscribe: true` (what this crate's
/// client sends, and draft-04's boolean form), the older `Subscribe: keep-alive`,
/// and `Prefer: subscribe`. Servers must accept all three — recognising only one
/// silently downgrades a subscribe into a plain `GET`, which the client then
/// fails to parse and retries forever.
///
/// `lookup` is called with a lowercase header name and returns its value.
pub fn wants_subscribe<'a>(lookup: impl Fn(&str) -> Option<&'a str>) -> bool {
    if let Some(v) = lookup("subscribe") {
        let v = v.trim();
        if v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("keep-alive") {
            return true;
        }
    }
    lookup("prefer").is_some_and(|v| v.to_ascii_lowercase().contains("subscribe"))
}

/// The versions a client says it already has, from the `Parents` request header.
///
/// A subscriber that sends this is resuming: the server should replay only what
/// is newer and skip the rest. Absent or unparseable means "send me everything",
/// which is always correct, just more bytes.
#[must_use]
pub fn resume_from<'a>(lookup: impl Fn(&str) -> Option<&'a str>) -> Vec<crate::types::Version> {
    lookup("parents")
        .and_then(|v| crate::protocol::parse_version_header(v).ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a header lookup over a fixed set of `(name, value)` pairs.
    fn given<'a>(
        pairs: &'a [(&'static str, &'static str)],
    ) -> impl Fn(&str) -> Option<&'static str> + 'a {
        move |name| pairs.iter().find(|(k, _)| *k == name).map(|(_, v)| *v)
    }

    #[test]
    fn subscription_response_is_209_with_http_history() {
        let res = SubscriptionResponse::new();
        assert_eq!(res.status, 209);
        let hs = res.headers();
        assert!(hs.contains(&("Content-Type", HTTP_HISTORY.to_string())));
        assert!(hs.iter().any(|(k, _)| *k == "Heartbeats"));
        // The old server advertised multipart; nothing may reintroduce it.
        assert!(!hs.iter().any(|(_, v)| v.contains("multipart")));
    }

    #[test]
    fn all_three_subscribe_spellings_are_accepted() {
        for pairs in [
            &[("subscribe", "true")][..],
            &[("subscribe", "keep-alive")][..],
            &[("prefer", "subscribe")][..],
            &[("prefer", "wait=10, subscribe")][..],
        ] {
            assert!(wants_subscribe(given(pairs)), "rejected {pairs:?}");
        }
    }

    #[test]
    fn a_plain_get_is_not_a_subscribe() {
        assert!(!wants_subscribe(given(&[("accept", "application/json")])));
        assert!(!wants_subscribe(given(&[])));
        assert!(!wants_subscribe(given(&[("subscribe", "false")])));
    }

    #[test]
    fn resume_from_reads_the_parents_header() {
        let versions = resume_from(given(&[("parents", "\"7\"")]));
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].to_string(), "7");
    }

    #[test]
    fn no_parents_header_means_replay_everything() {
        assert!(resume_from(given(&[])).is_empty());
        assert!(resume_from(given(&[("parents", "")])).is_empty());
    }

    #[test]
    fn multiple_parents_are_all_returned() {
        let versions = resume_from(given(&[("parents", "\"3\", \"7\"")]));
        assert_eq!(versions.len(), 2);
    }

    #[test]
    fn an_unrecognised_parent_is_the_servers_problem_not_a_parse_error() {
        // Version ids are opaque strings, so anything well-formed parses. A server
        // that doesn't recognise the version should replay from the start rather
        // than reject the subscribe.
        assert_eq!(resume_from(given(&[("parents", "\"nonsense\"")])).len(), 1);
    }
}
