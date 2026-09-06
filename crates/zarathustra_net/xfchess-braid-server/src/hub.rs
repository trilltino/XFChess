//! Registry and fan-out hub for live Braid resources.
//!
//! Each write is sent to local HTTP subscribers and, when configured, the
//! optional gossip sink.

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

/// Receives each published update for another transport such as P2P gossip.
/// The callback must return quickly because writes invoke it synchronously.
pub type GossipSink = Arc<dyn Fn(&str, &BraidUpdate) + Send + Sync>;

/// Shared registry of live resources and subscriber channels.
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

    /// Registers a patched document with an initial JSON value.
    pub fn register_doc(&self, path: impl Into<String>, initial: Value) {
        let path = path.into();
        debug!("[braid-hub] register_doc {}", path);
        self.inner
            .write()
            .insert(path, ResourceEntry::Doc(PatchedDoc::new(initial)));
    }

    /// Registers an empty append log, replacing any existing resource.
    pub fn register_log(&self, path: impl Into<String>) {
        let path = path.into();
        debug!("[braid-hub] register_log {}", path);
        self.inner
            .write()
            .insert(path, ResourceEntry::Log(AppendLog::new()));
    }

    /// Registers an append log only when `path` is absent.
    pub fn ensure_log(&self, path: &str) -> bool {
        if self.inner.read().contains_key(path) {
            return false;
        }
        self.register_log(path.to_string());
        true
    }

    /// Returns whether a resource is registered at `path`.
    pub fn has(&self, path: &str) -> bool {
        self.inner.read().contains_key(path)
    }

    /// Returns the current JSON state of a resource.
    pub async fn current_json(&self, path: &str) -> Option<Value> {
        let entry = self.inner.read().get(path)?.clone();
        Some(match entry {
            ResourceEntry::Doc(doc) => doc.snapshot().0,
            ResourceEntry::Log(log) => log.snapshot().0,
        })
    }

    /// Returns the current snapshot and a live update receiver.
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

    /// Applies a JSON Patch to a patched document.
    pub fn patch(&self, path: &str, patch: Patch) {
        if let Some(ResourceEntry::Doc(doc)) = self.inner.read().get(path).cloned() {
            match doc.apply(patch) {
                Ok(update) => self.fan_out(path, &update),
                Err(e) => tracing::warn!("[braid-hub] patch failed on {}: {}", path, e),
            }
        }
    }

    /// Replaces a patched document and broadcasts the new snapshot.
    pub fn replace(&self, path: &str, new_doc: Value) {
        if let Some(ResourceEntry::Doc(doc)) = self.inner.read().get(path).cloned() {
            let update = doc.replace(new_doc);
            self.fan_out(path, &update);
        }
    }

    /// Appends an entry to an append log.
    pub fn append(&self, path: &str, entry: Value) {
        if let Some(ResourceEntry::Log(log)) = self.inner.read().get(path).cloned() {
            let update = log.append(entry);
            self.fan_out(path, &update);
        }
    }

    /// Ensures the standard resources for a tournament exist.
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

        let results = format!("tournament/{}/results", tid);
        if !self.inner.read().contains_key(results.as_str()) {
            self.register_log(results);
        }
    }

    /// Ensures the pairings resource for a round exists.
    pub fn ensure_pairings(&self, tournament_id: u64, round: u8) {
        let path = format!("tournament/{}/pairings/{}", tournament_id, round);
        if !self.inner.read().contains_key(&path) {
            self.register_doc(path, Value::Array(Vec::new()));
        }
    }
}
