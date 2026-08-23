//! Central registry of all Braid resources.
//!
//! A [`ResourceHub`] maps resource paths (e.g. `tournament/42/standings`) to
//! either a [`PatchedDoc`] or an [`AppendLog`]. Callers obtain handles to
//! individual resources and push updates; the hub fans those out to all active
//! subscribers.
//!
//! # One write, two fan-outs
//!
//! Every fact enters through exactly one hub write. The hub then delivers the
//! resulting [`BraidUpdate`] to:
//!
//! 1. local subscribers, over each resource's `tokio::broadcast` channel —
//!    what the HTTP `209` handler streams; and
//! 2. the optional **gossip sink** ([`ResourceHub::set_gossip_sink`]) — what
//!    carries the same update to P2P peers.
//!
//! Before the sink existed, the backend wrote tournament facts twice: once
//! into the hub, and once as a separate tagged-JSON gossip broadcast that had
//! no version, no parents, and no way for a late peer to catch up. The two
//! could and did drift. Now the gossip payload *is* the Braid update, so both
//! transports carry the same versioned bytes.

use crate::resource::{
    protocol::BraidUpdate,
    store::{AppendLog, PatchedDoc},
};
use json_patch::Patch;
use parking_lot::RwLock;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::debug;

#[derive(Clone)]
enum ResourceEntry {
    Doc(PatchedDoc),
    Log(AppendLog),
}

/// A destination for every update the hub publishes, beyond its own local
/// subscribers — in practice, the P2P gossip transport.
///
/// Called synchronously with the resource path and the update just published.
/// Implementations must not block: the hub's mutators are sync and run on
/// whatever task performed the write. Hand the update to a channel or spawn a
/// task and return.
pub type GossipSink = Arc<dyn Fn(&str, &BraidUpdate) + Send + Sync>;

/// Shared registry of all live resources and their subscriber channels.
#[derive(Clone, Default)]
pub struct ResourceHub {
    inner: Arc<RwLock<HashMap<String, ResourceEntry>>>,
    gossip_sink: Arc<RwLock<Option<GossipSink>>>,
}

impl ResourceHub {
    pub fn new() -> Self {
        Self::default()
    }

    /// Install the sink that mirrors every published update onto a second
    /// transport. Replaces any previously installed sink.
    pub fn set_gossip_sink(&self, sink: GossipSink) {
        *self.gossip_sink.write() = Some(sink);
    }

    /// Hand one published update to the gossip sink, if one is installed.
    fn fan_out(&self, path: &str, update: &BraidUpdate) {
        let sink = self.gossip_sink.read().clone();
        if let Some(sink) = sink {
            sink(path, update);
        }
    }

    // ── Registration ─────────────────────────────────────────────────────────

    /// Register a patched-doc resource with an initial JSON value.
    pub fn register_doc(&self, path: impl Into<String>, initial: Value) {
        let path = path.into();
        debug!("[braid-hub] register_doc {}", path);
        self.inner
            .write()
            .insert(path, ResourceEntry::Doc(PatchedDoc::new(initial)));
    }

    /// Register an append-log resource (starts empty).
    ///
    /// Replaces any existing resource at `path` — which drops its history and
    /// its subscribers. Use [`Self::ensure_log`] unless you mean that.
    pub fn register_log(&self, path: impl Into<String>) {
        let path = path.into();
        debug!("[braid-hub] register_log {}", path);
        self.inner
            .write()
            .insert(path, ResourceEntry::Log(AppendLog::new()));
    }

    /// Register an append-log only if `path` is not already registered.
    ///
    /// Returns `true` when it created one — the caller's cue to backfill it
    /// from durable storage before anyone subscribes.
    pub fn ensure_log(&self, path: &str) -> bool {
        if self.inner.read().contains_key(path) {
            return false;
        }
        self.register_log(path.to_string());
        true
    }

    /// Whether a resource is registered at `path`.
    pub fn has(&self, path: &str) -> bool {
        self.inner.read().contains_key(path)
    }

    // ── Read ─────────────────────────────────────────────────────────────────

    /// Current JSON state of a resource.
    pub async fn current_json(&self, path: &str) -> Option<Value> {
        let entry = self.inner.read().get(path)?.clone();
        Some(match entry {
            ResourceEntry::Doc(doc) => doc.snapshot().0,
            ResourceEntry::Log(log) => log.snapshot().0,
        })
    }

    /// Subscribe to a resource: returns (snapshot, live receiver).
    pub async fn subscribe(
        &self,
        path: &str,
    ) -> Option<(BraidUpdate, broadcast::Receiver<BraidUpdate>)> {
        let entry = self.inner.read().get(path)?.clone();
        Some(match entry {
            ResourceEntry::Doc(doc) => doc.subscribe(),
            ResourceEntry::Log(log) => log.subscribe(),
        })
    }

    // ── Mutation ─────────────────────────────────────────────────────────────

    /// Apply a JSON Patch to a patched-doc resource.
    pub fn patch(&self, path: &str, patch: Patch) {
        if let Some(ResourceEntry::Doc(doc)) = self.inner.read().get(path).cloned() {
            match doc.apply(patch) {
                Ok(update) => self.fan_out(path, &update),
                Err(e) => tracing::warn!("[braid-hub] patch failed on {}: {}", path, e),
            }
        }
    }

    /// Replace a patched-doc resource's entire document.
    pub fn replace(&self, path: &str, new_doc: Value) {
        if let Some(ResourceEntry::Doc(doc)) = self.inner.read().get(path).cloned() {
            let update = doc.replace(new_doc);
            self.fan_out(path, &update);
        }
    }

    /// Append an entry to an append-log resource.
    pub fn append(&self, path: &str, entry: Value) {
        if let Some(ResourceEntry::Log(log)) = self.inner.read().get(path).cloned() {
            let update = log.append(entry);
            self.fan_out(path, &update);
        }
    }

    // ── Helpers for tournament resources ────────────────────────────────────

    /// Ensure the standard resources for a tournament exist.
    ///
    /// `standings` starts as an array rather than an object: it is always a
    /// ranked list, and a subscriber that connects before the first result is
    /// recorded should get an empty list, not an empty object it cannot parse.
    pub fn ensure_tournament(&self, tournament_id: u64) {
        let tid = tournament_id;
        let docs = [
            format!("tournament/{}/meta", tid),
            format!("tournament/{}/schedule-status", tid),
            format!("tournament/{}/roster", tid),
        ];
        for path in &docs {
            if !self.inner.read().contains_key(path.as_str()) {
                self.register_doc(path.clone(), Value::Object(Default::default()));
            }
        }

        let standings = format!("tournament/{}/standings", tid);
        if !self.inner.read().contains_key(standings.as_str()) {
            self.register_doc(standings, Value::Array(Vec::new()));
        }

        // Results are append-only — one entry per finished board, in the
        // order they were recorded. A late subscriber replays the whole log.
        let results = format!("tournament/{}/results", tid);
        if !self.inner.read().contains_key(results.as_str()) {
            self.register_log(results);
        }
    }

    /// Ensure the pairings resource for a round exists.
    pub fn ensure_pairings(&self, tournament_id: u64, round: u8) {
        let path = format!("tournament/{}/pairings/{}", tournament_id, round);
        if !self.inner.read().contains_key(&path) {
            self.register_doc(path, Value::Array(Vec::new()));
        }
    }
}
