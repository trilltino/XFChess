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

pub type GossipSink = Arc<dyn Fn(&str, &BraidUpdate) + Send + Sync>;

#[derive(Clone, Default)]
pub struct ResourceHub {
    inner: Arc<RwLock<HashMap<String, ResourceEntry>>>,
    gossip_sink: Arc<RwLock<Option<GossipSink>>>,
}

impl ResourceHub {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_gossip_sink(&self, sink: GossipSink) {
        *self.gossip_sink.write() = Some(sink);
    }

    fn fan_out(&self, path: &str, update: &BraidUpdate) {
        let sink = self.gossip_sink.read().clone();
        if let Some(sink) = sink {
            sink(path, update);
        }
    }

    pub fn register_doc(&self, path: impl Into<String>, initial: Value) {
        let path = path.into();
        debug!("[braid-hub] register_doc {}", path);
        self.inner
            .write()
            .insert(path, ResourceEntry::Doc(PatchedDoc::new(initial)));
    }

    pub fn register_log(&self, path: impl Into<String>) {
        let path = path.into();
        debug!("[braid-hub] register_log {}", path);
        self.inner
            .write()
            .insert(path, ResourceEntry::Log(AppendLog::new()));
    }

    pub fn ensure_log(&self, path: &str) -> bool {
        if let std::collections::hash_map::Entry::Vacant(entry) =
            self.inner.write().entry(path.to_owned())
        {
            entry.insert(ResourceEntry::Log(AppendLog::new()));
            true
        } else {
            false
        }
    }

    pub fn has(&self, path: &str) -> bool {
        self.inner.read().contains_key(path)
    }

    pub async fn current_json(&self, path: &str) -> Option<Value> {
        let entry = self.inner.read().get(path)?.clone();
        Some(match entry {
            ResourceEntry::Doc(doc) => doc.snapshot().0,
            ResourceEntry::Log(log) => log.snapshot().0,
        })
    }

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

    pub fn patch(&self, path: &str, patch: Patch) {
        let entry = self.inner.read().get(path).cloned();
        if let Some(ResourceEntry::Doc(doc)) = entry {
            match doc.apply(patch) {
                Ok(update) => self.fan_out(path, &update),
                Err(e) => tracing::warn!("[braid-hub] patch failed on {}: {}", path, e),
            }
        }
    }

    pub fn replace(&self, path: &str, new_doc: Value) {
        let entry = self.inner.read().get(path).cloned();
        if let Some(ResourceEntry::Doc(doc)) = entry {
            let update = doc.replace(new_doc);
            self.fan_out(path, &update);
        }
    }

    pub fn append(&self, path: &str, entry: Value) {
        let resource = self.inner.read().get(path).cloned();
        if let Some(ResourceEntry::Log(log)) = resource {
            let update = log.append(entry);
            self.fan_out(path, &update);
        }
    }

    pub fn ensure_tournament(&self, tournament_id: u64) {
        let tid = tournament_id;
        let docs = [
            format!("tournament/{}/meta", tid),
            format!("tournament/{}/schedule-status", tid),
            format!("tournament/{}/roster", tid),
        ];
        let mut resources = self.inner.write();
        for path in docs {
            resources.entry(path).or_insert_with(|| {
                ResourceEntry::Doc(PatchedDoc::new(Value::Object(Default::default())))
            });
        }

        let standings = format!("tournament/{}/standings", tid);
        resources
            .entry(standings)
            .or_insert_with(|| ResourceEntry::Doc(PatchedDoc::new(Value::Array(Vec::new()))));

        let results = format!("tournament/{}/results", tid);
        resources
            .entry(results)
            .or_insert_with(|| ResourceEntry::Log(AppendLog::new()));
    }

    pub fn ensure_pairings(&self, tournament_id: u64, round: u8) {
        let path = format!("tournament/{}/pairings/{}", tournament_id, round);
        self.inner
            .write()
            .entry(path)
            .or_insert_with(|| ResourceEntry::Doc(PatchedDoc::new(Value::Array(Vec::new()))));
    }
}
