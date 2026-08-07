//! Utility functions for the Braid HTTP client.

//! Small shared helpers for the client: header value parsing, message → update
//! conversion, and the runtime shims that let the same code run on tokio and in
//! a browser.

use crate::client::parser::Message;
use crate::error::{BraidError, Result};
use crate::protocol;
use crate::types::{Update, Version};
use std::time::Duration;

pub fn parse_content_range(header: &str) -> Result<(String, String)> {
    protocol::parse_content_range(header)
}

pub fn format_content_range(unit: &str, range: &str) -> String {
    protocol::format_content_range(unit, range)
}

pub fn parse_heartbeat(value: &str) -> Result<Duration> {
    let trimmed = value.trim();
    let num_str = if let Some(s) = trimmed.strip_suffix("ms") {
        s
    } else if let Some(s) = trimmed.strip_suffix('s') {
        s
    } else {
        trimmed
    };

    let num: f64 = num_str
        .parse()
        .map_err(|_| BraidError::HeaderParse(format!("Invalid heartbeat: {}", value)))?;
    Ok(Duration::from_secs_f64(num))
}

/// Convert one parsed wire [`Message`] into the public [`Update`] type.
pub fn message_to_update(msg: Message) -> Update {
    let version = extract_version(&msg.headers).unwrap_or_else(|| {
        // An update with no Version can't take part in causal ordering. Give it a
        // placeholder so it is still delivered, and say so once, here — callers
        // that care about ordering can check `Update::primary_version`.
        tracing::warn!(
            "[BraidHTTP] update from {} had no Version header — applying a placeholder, causal ordering may be affected",
            msg.url.as_deref().unwrap_or("unknown"),
        );
        Version::new("temp-0")
    });

    // Snapshot vs. patch is decided by the wire, not by emptiness: a `Patches: 0`
    // update has an empty patch list and is still a patch update.
    let mut builder = match (msg.body, msg.patches) {
        (_, Some(patches)) => Update::patched(version, patches),
        (Some(body), None) => Update::snapshot(version, String::from_utf8_lossy(&body).into_owned()),
        (None, None) => Update::snapshot(version, ""),
    };

    if let Some(parents) = extract_parents(&msg.headers) {
        for parent in parents {
            builder = builder.with_parent(parent);
        }
    }

    if let Some(merge_type) = msg.headers.get("merge-type") {
        builder = builder.with_merge_type(merge_type.clone());
    }

    builder.url = msg.url;
    builder
}

fn extract_version(headers: &std::collections::BTreeMap<String, String>) -> Option<Version> {
    let version = headers
        .get("current-version")
        .or_else(|| headers.get("version"))
        .or_else(|| headers.get("Version"))
        .or_else(|| headers.get("Current-Version"))
        .and_then(|v| {
            let trimmed = v.trim();
            if trimmed.is_empty() || trimmed == "\"\"" {
                return None;
            }
            protocol::parse_version_header(v).ok()
        })
        .and_then(|mut v| v.pop());

    if version.is_none() {
        // The caller (`message_to_update`'s `unwrap_or_else`) already logs a
        // single, human-readable warning for this — don't double-log the
        // same event here. Headers are still useful when actually
        // debugging a protocol mismatch, so keep them at trace level.
        tracing::trace!("[BraidHTTP] no version header found. Headers were: {:?}", headers);
    } else {
        tracing::debug!("[BraidHTTP] Parsed version: {:?}", version);
    }
    version
}

fn extract_parents(headers: &std::collections::BTreeMap<String, String>) -> Option<Vec<Version>> {
    let parents = headers
        .get("parents")
        .and_then(|v| protocol::parse_version_header(v).ok());

    if parents.is_none() {
        tracing::debug!(
            "[BraidHTTP] Parents header missing or failed to parse. Headers: {:?}",
            headers
        );
    } else {
        tracing::debug!("[BraidHTTP] Parsed parents: {:?}", parents);
    }
    parents
}

#[cfg(not(target_arch = "wasm32"))]
pub fn spawn_task<F>(future: F)
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    tokio::spawn(future);
}

#[cfg(target_arch = "wasm32")]
pub fn spawn_task<F>(future: F)
where
    F: std::future::Future<Output = ()> + 'static,
{
    wasm_bindgen_futures::spawn_local(future);
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn sleep(duration: Duration) {
    tokio::time::sleep(duration).await;
}

#[cfg(target_arch = "wasm32")]
pub async fn sleep(duration: Duration) {
    gloo_timers::future::sleep(duration).await;
}
