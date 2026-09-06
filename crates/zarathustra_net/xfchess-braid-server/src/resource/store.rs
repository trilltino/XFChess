//! Versioned JSON document and append-log resource stores.

use crate::resource::protocol::{BraidUpdate, Version};
use json_patch::Patch;
use parking_lot::RwLock;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::debug;

/// Channel capacity: at most this many buffered updates per subscriber.
const BROADCAST_CAPACITY: usize = 256;

/// A versioned JSON document that accepts RFC 6902 patches.
///
/// Subscribers receive the current snapshot on connect then every subsequent
/// patch as a [`BraidUpdate`].
#[derive(Clone)]
pub struct PatchedDoc {
    inner: Arc<RwLock<PatchedDocInner>>,
    tx: broadcast::Sender<BraidUpdate>,
}

struct PatchedDocInner {
    doc: Value,
    version: Version,
}

impl PatchedDoc {
    pub fn new(initial: Value) -> Self {
        let (tx, _) = broadcast::channel(BROADCAST_CAPACITY);
        Self {
            inner: Arc::new(RwLock::new(PatchedDocInner {
                doc: initial,
                version: 0,
            })),
            tx,
        }
    }

    /// Returns the current snapshot.
    pub fn snapshot(&self) -> (Value, Version) {
        let g = self.inner.read();
        (g.doc.clone(), g.version)
    }

    /// Returns the current snapshot and a live update receiver.
    pub fn subscribe(&self) -> (BraidUpdate, broadcast::Receiver<BraidUpdate>) {
        let rx = self.tx.subscribe();
        let (doc, ver) = self.snapshot();
        let snap = BraidUpdate::snapshot(ver, doc);
        (snap, rx)
    }

    /// Applies a JSON Patch and broadcasts the resulting update.
    pub fn apply(&self, patches: Patch) -> Result<BraidUpdate, json_patch::PatchError> {
        let mut g = self.inner.write();
        let parent = g.version;
        json_patch::patch(&mut g.doc, &patches)?;
        g.version += 1;
        let version = g.version;

        let patch_value = serde_json::to_value(&patches).unwrap_or(Value::Array(vec![]));
        let update = BraidUpdate::patch(version, parent, patch_value);
        debug!("[braid] patched_doc v{} → v{}", parent, version);
        let _ = self.tx.send(update.clone());
        Ok(update)
    }

    /// Replaces the document and broadcasts a snapshot.
    pub fn replace(&self, new_doc: Value) -> BraidUpdate {
        let mut g = self.inner.write();
        let parent = g.version;
        g.doc = new_doc.clone();
        g.version += 1;
        let version = g.version;
        let update = BraidUpdate::snapshot(version, new_doc);
        debug!("[braid] patched_doc replaced v{} → v{}", parent, version);
        let _ = self.tx.send(update.clone());
        update
    }
}

/// An append-only log of JSON values.
///
/// Subscribers receive all existing entries on connect, then each new entry
/// as a [`BraidUpdate`].
#[derive(Clone)]
pub struct AppendLog {
    inner: Arc<RwLock<AppendLogInner>>,
    tx: broadcast::Sender<BraidUpdate>,
}

struct AppendLogInner {
    entries: Vec<Value>,
    version: Version,
}

impl AppendLog {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(BROADCAST_CAPACITY);
        Self {
            inner: Arc::new(RwLock::new(AppendLogInner {
                entries: Vec::new(),
                version: 0,
            })),
            tx,
        }
    }

    /// Returns all current entries.
    pub fn snapshot(&self) -> (Value, Version) {
        let g = self.inner.read();
        (Value::Array(g.entries.clone()), g.version)
    }

    /// Returns the snapshot and a live receiver.
    pub fn subscribe(&self) -> (BraidUpdate, broadcast::Receiver<BraidUpdate>) {
        let rx = self.tx.subscribe();
        let (entries, ver) = self.snapshot();
        let snap = BraidUpdate::snapshot(ver, entries);
        (snap, rx)
    }

    /// Appends an entry and broadcasts a single-element array patch.
    pub fn append(&self, entry: Value) -> BraidUpdate {
        let mut g = self.inner.write();
        g.entries.push(entry.clone());
        let parent = g.version;
        g.version += 1;
        let version = g.version;

        let patch_value = serde_json::json!([
            {"op": "add", "path": "/-", "value": entry}
        ]);
        let update = BraidUpdate::patch(version, parent, patch_value);
        debug!("[braid] append_log appended v{}", version);
        let _ = self.tx.send(update.clone());
        update
    }
}

impl Default for AppendLog {
    fn default() -> Self {
        Self::new()
    }
}
