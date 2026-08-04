//! Central registry of all Braid resources.
//!
//! A [`ResourceHub`] maps resource paths (e.g. `tournament/42/standings`) to
//! either a [`PatchedDoc`] or an [`AppendLog`]. Callers obtain handles to
//! individual resources and push updates; the hub fans those out to all active
//! subscribers.

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

/// Shared registry of all live resources and their subscriber channels.
#[derive(Clone, Default)]
pub struct ResourceHub {
    inner: Arc<RwLock<HashMap<String, ResourceEntry>>>,
}

impl ResourceHub {
    pub fn new() -> Self {
        Self::default()
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
    pub fn register_log(&self, path: impl Into<String>) {
        let path = path.into();
        debug!("[braid-hub] register_log {}", path);
        self.inner
            .write()
            .insert(path, ResourceEntry::Log(AppendLog::new()));
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
            if let Err(e) = doc.apply(patch) {
                tracing::warn!("[braid-hub] patch failed on {}: {}", path, e);
            }
        }
    }

    /// Replace a patched-doc resource's entire document.
    pub fn replace(&self, path: &str, new_doc: Value) {
        if let Some(ResourceEntry::Doc(doc)) = self.inner.read().get(path).cloned() {
            doc.replace(new_doc);
        }
    }

    /// Append an entry to an append-log resource.
    pub fn append(&self, path: &str, entry: Value) {
        if let Some(ResourceEntry::Log(log)) = self.inner.read().get(path).cloned() {
            log.append(entry);
        }
    }

    // ── Helpers for tournament resources ────────────────────────────────────

    /// Ensure all five standard resources for a tournament exist.
    pub fn ensure_tournament(&self, tournament_id: u64) {
        let tid = tournament_id;
        let paths = [
            format!("tournament/{}/meta", tid),
            format!("tournament/{}/schedule-status", tid),
            format!("tournament/{}/roster", tid),
            format!("tournament/{}/standings", tid),
        ];
        let w = self.inner.read();
        drop(w);
        for path in &paths {
            if !self.inner.read().contains_key(path.as_str()) {
                self.register_doc(path.clone(), Value::Object(Default::default()));
            }
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
