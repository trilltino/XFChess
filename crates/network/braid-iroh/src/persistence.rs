use crate::store::Histories;
use crate::store::ResourceStore;
use std::path::Path;
use std::{
    path::PathBuf,
    sync::{atomic::Ordering, Arc},
    time::Duration,
};

pub(crate) struct FlushTask {
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<()>,
}

impl FlushTask {
    pub(crate) fn spawn(dir: PathBuf, resources: Arc<ResourceStore>) -> Self {
        let (stop, mut stopped) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tokio::select! {
                    biased;
                    _ = &mut stopped => break,
                    _ = interval.tick() => {},
                }
                if resources.dirty.swap(false, Ordering::AcqRel) {
                    if let Err(error) = save(&dir, &resources.snapshot().await).await {
                        resources.dirty.store(true, Ordering::Release);
                        tracing::error!("Resource persistence failed: {error}");
                    }
                }
            }
        });
        Self {
            stop: Some(stop),
            task,
        }
    }

    pub(crate) async fn shutdown(&mut self) -> anyhow::Result<()> {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        (&mut self.task).await?;
        Ok(())
    }
}

impl Drop for FlushTask {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub(crate) async fn load(data_dir: &Path) -> anyhow::Result<Histories> {
    let path = data_dir.join("resources.json");
    match tokio::fs::read(path).await {
        Ok(data) => Ok(serde_json::from_slice(&data)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Histories::new()),
        Err(e) => Err(e.into()),
    }
}

pub(crate) async fn save(data_dir: &Path, histories: &Histories) -> anyhow::Result<()> {
    let temporary = data_dir.join("resources.json.tmp");
    let data = serde_json::to_vec(histories)?;
    tokio::fs::write(&temporary, data).await?;
    tokio::fs::rename(temporary, data_dir.join("resources.json")).await?;
    Ok(())
}
