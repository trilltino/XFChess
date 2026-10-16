use braid_http::Update;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::{broadcast, RwLock};

use crate::subscription::SubscriptionManager;

const HISTORY_LIMIT: usize = 500;
pub(crate) type Histories = HashMap<String, VecDeque<Update>>;

struct Resource {
    history: VecDeque<Update>,
    sender: broadcast::Sender<Update>,
}

impl Default for Resource {
    fn default() -> Self {
        Self {
            history: VecDeque::new(),
            sender: broadcast::channel(256).0,
        }
    }
}

pub(crate) struct ResourceStore {
    resources: RwLock<HashMap<String, Resource>>,
    pub(crate) dirty: AtomicBool,
}

impl ResourceStore {
    pub(crate) fn new(histories: Histories) -> Self {
        let mut resources = HashMap::new();
        for (url, mut history) in histories {
            while history.len() > HISTORY_LIMIT {
                history.pop_front();
            }
            resources.insert(
                SubscriptionManager::normalize_url(&url),
                Resource {
                    history,
                    ..Resource::default()
                },
            );
        }
        Self {
            resources: RwLock::new(resources),
            dirty: AtomicBool::new(false),
        }
    }

    pub(crate) async fn append(&self, url: &str, update: Update) {
        let url = SubscriptionManager::normalize_url(url);
        let mut resources = self.resources.write().await;
        let resource = resources.entry(url).or_default();
        if !update.version.is_empty()
            && resource
                .history
                .iter()
                .any(|old| old.version == update.version)
        {
            return;
        }
        resource.history.push_back(update.clone());
        if resource.history.len() > HISTORY_LIMIT {
            resource.history.pop_front();
        }
        self.dirty.store(true, Ordering::Release);
        let _ = resource.sender.send(update);
    }

    pub(crate) async fn history(&self, url: &str) -> Vec<Update> {
        self.resources
            .read()
            .await
            .get(&SubscriptionManager::normalize_url(url))
            .map(|r| r.history.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub(crate) async fn latest(&self, url: &str) -> Option<Update> {
        self.resources
            .read()
            .await
            .get(&SubscriptionManager::normalize_url(url))?
            .history
            .back()
            .cloned()
    }

    pub(crate) async fn version(&self, url: &str, version_id: &str) -> Option<Update> {
        self.resources
            .read()
            .await
            .get(&SubscriptionManager::normalize_url(url))?
            .history
            .iter()
            .find(|u| u.version.iter().any(|v| v.to_string() == version_id))
            .cloned()
    }

    #[cfg(feature = "proxy")]
    pub(crate) async fn subscribe(
        &self,
        url: &str,
        parents: &[braid_http::Version],
    ) -> braid_http::Result<(Vec<Update>, broadcast::Receiver<Update>)> {
        let mut resources = self.resources.write().await;
        let resource = resources
            .entry(SubscriptionManager::normalize_url(url))
            .or_default();
        let history = resource.history.iter().cloned().collect();
        let receiver = resource.sender.subscribe();
        drop(resources);
        Ok((resume_history(history, parents)?, receiver))
    }

    pub(crate) async fn snapshot(&self) -> Histories {
        self.resources
            .read()
            .await
            .iter()
            .filter(|(_, r)| !r.history.is_empty())
            .map(|(url, r)| (url.clone(), r.history.clone()))
            .collect()
    }
}

#[cfg(feature = "proxy")]
fn resume_history(
    history: Vec<Update>,
    parents: &[braid_http::Version],
) -> braid_http::Result<Vec<Update>> {
    use std::collections::HashSet;
    if parents.is_empty() {
        return Ok(history);
    }
    let graph: HashMap<_, _> = history
        .iter()
        .flat_map(|update| {
            update
                .version
                .iter()
                .cloned()
                .map(move |version| (version, update))
        })
        .collect();
    if parents.iter().any(|head| !graph.contains_key(head)) {
        return Err(braid_http::BraidError::HistoryDropped);
    }
    let mut known = HashSet::new();
    let mut pending = parents.to_vec();
    while let Some(version) = pending.pop() {
        if known.insert(version.clone()) {
            if let Some(update) = graph.get(&version) {
                pending.extend(update.parents.iter().cloned());
                pending.extend(
                    update
                        .version
                        .iter()
                        .filter(|head| !known.contains(*head))
                        .cloned(),
                );
            }
        }
    }
    Ok(history
        .into_iter()
        .filter(|update| {
            update.version.is_empty() || update.version.iter().any(|head| !known.contains(head))
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use braid_http::Version;

    #[cfg(feature = "proxy")]
    #[tokio::test]
    async fn resuming_a_frontier_keeps_unseen_branches_and_live_updates() {
        let store = ResourceStore::new(HashMap::new());
        for (version, parents) in [
            ("root", vec![]),
            ("a1", vec!["root"]),
            ("b1", vec!["root"]),
            ("a2", vec!["a1"]),
        ] {
            store
                .append(
                    "game/1/moves",
                    Update::snapshot(Version::new(version), version)
                        .with_parents(parents.into_iter().map(Version::new).collect()),
                )
                .await;
        }
        let (history, _) = store
            .subscribe("game/1/moves", &[Version::new("a1")])
            .await
            .unwrap();
        assert_eq!(
            history
                .iter()
                .map(|u| u.version[0].to_string())
                .collect::<Vec<_>>(),
            ["b1", "a2"]
        );
        let (history, mut receiver) = store
            .subscribe("game/1/moves", &[Version::new("a1"), Version::new("b1")])
            .await
            .unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].version, [Version::new("a2")]);
        let merged = Update::snapshot(Version::new("merged"), "merged")
            .with_parents(vec![Version::new("a2"), Version::new("b1")]);
        store.append("game/1/moves", merged.clone()).await;
        assert_eq!(receiver.recv().await.unwrap().version, merged.version);
        assert!(store
            .subscribe("game/1/moves", &[Version::new("missing")])
            .await
            .is_err());
        assert!(store
            .subscribe("game/1/moves", &[Version::new("merged")])
            .await
            .unwrap()
            .0
            .is_empty());
    }

    #[tokio::test]
    async fn history_is_normalized_bounded_and_retries_are_idempotent() {
        let store = ResourceStore::new(HashMap::new());
        for i in 0..510 {
            store
                .append(
                    "game/1/moves/",
                    Update::snapshot(Version::new(i.to_string()), "move"),
                )
                .await;
        }
        store
            .append(
                "/game/1/moves",
                Update::snapshot(Version::new("509"), "move"),
            )
            .await;
        let history = store.history("game/1/moves").await;
        assert_eq!(history.len(), 500);
        assert_eq!(history[0].version[0].to_string(), "10");
        assert_eq!(history.last().unwrap().version[0].to_string(), "509");
        #[cfg(feature = "proxy")]
        assert!(matches!(
            store.subscribe("game/1/moves", &[Version::new("9")]).await,
            Err(braid_http::BraidError::HistoryDropped)
        ));
    }
}
