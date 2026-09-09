use crate::protocol::constants::status;
use std::time::Duration;

pub const HTTP_HISTORY: &str = "application/http-history";

pub const DEFAULT_HEARTBEAT_SECS: u64 = 20;

#[derive(Debug, Clone)]
pub struct SubscriptionResponse {
    pub status: u16,
    pub heartbeat_interval: Duration,
}

impl SubscriptionResponse {
    #[must_use]
    pub fn new() -> Self {
        Self::with_heartbeat_secs(DEFAULT_HEARTBEAT_SECS)
    }

    #[must_use]
    pub fn with_heartbeat_secs(secs: u64) -> Self {
        Self {
            status: status::SUBSCRIPTION,
            heartbeat_interval: Duration::from_secs(secs),
        }
    }

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

pub fn wants_subscribe<'a>(lookup: impl Fn(&str) -> Option<&'a str>) -> bool {
    if let Some(v) = lookup("subscribe") {
        let v = v.trim();
        if v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("keep-alive") {
            return true;
        }
    }
    lookup("prefer").is_some_and(|v| v.to_ascii_lowercase().contains("subscribe"))
}

#[must_use]
pub fn resume_from<'a>(lookup: impl Fn(&str) -> Option<&'a str>) -> Vec<crate::types::Version> {
    lookup("parents")
        .and_then(|v| crate::protocol::parse_version_header(v).ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

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
