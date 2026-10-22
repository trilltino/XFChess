use crate::ChessMessage;
#[cfg(test)]
use anyhow::Context as _;
use braid_http::Version;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PendingWrite {
    pub stream: String,
    pub sender: String,
    pub message: ChessMessage,
    pub version: String,
    pub parent: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Checkpoint {
    pub versions: Vec<Version>,
    pub fen: Option<String>,
}

#[derive(Default, Serialize, Deserialize)]
struct State {
    pending: Vec<PendingWrite>,
    checkpoints: BTreeMap<String, Checkpoint>,
}

/// One game on one backend and device. Credentials are supplied by the current session.
#[derive(Clone)]
pub struct Journal {
    directory: PathBuf,
}

impl Journal {
    pub fn open(root: &Path, scope: &str) -> io::Result<Self> {
        use sha2::{Digest, Sha256};
        let directory = root.join(hex::encode(Sha256::digest(scope.as_bytes())));
        std::fs::create_dir_all(&directory)?;
        Ok(Self { directory })
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    fn locked(&self) -> io::Result<(File, State)> {
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(self.directory.join("lock"))?;
        lock.lock()?;
        let state = match std::fs::read(self.directory.join("state.json")) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(io::Error::other)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => State::default(),
            Err(error) => return Err(error),
        };
        Ok((lock, state))
    }

    fn change(&self, change: impl FnOnce(&mut State) -> io::Result<()>) -> io::Result<()> {
        let (_lock, mut state) = self.locked()?;
        change(&mut state)?;
        let mut file = tempfile::NamedTempFile::new_in(&self.directory)?;
        file.write_all(&serde_json::to_vec(&state).map_err(io::Error::other)?)?;
        file.as_file().sync_all()?;
        file.persist(self.directory.join("state.json"))
            .map_err(io::Error::other)?;
        #[cfg(unix)]
        File::open(&self.directory)?.sync_all()?;
        Ok(())
    }

    pub fn enqueue(&self, write: PendingWrite) -> io::Result<()> {
        self.change(|state| {
            if let Some(existing) = state
                .pending
                .iter()
                .find(|entry| entry.version == write.version)
            {
                if existing.sender != write.sender
                    || existing.stream != write.stream
                    || serde_json::to_value(&existing.message)?
                        != serde_json::to_value(&write.message)?
                {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "version reused for a different event",
                    ));
                }
            } else {
                state.pending.push(write);
            }
            Ok(())
        })
    }

    pub fn pending(&self) -> io::Result<Vec<PendingWrite>> {
        Ok(self.locked()?.1.pending)
    }

    pub fn reparent(&self, version: &str, parent: &str) -> io::Result<()> {
        self.change(|state| {
            let entry = state
                .pending
                .iter_mut()
                .find(|entry| entry.version == version)
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "pending event missing"))?;
            entry.parent = parent.to_owned();
            Ok(())
        })
    }

    pub fn confirm_write(&self, version: &str) -> io::Result<()> {
        self.change(|state| {
            state.pending.retain(|entry| entry.version != version);
            Ok(())
        })
    }

    pub fn checkpoint(&self, stream: &str) -> io::Result<Checkpoint> {
        Ok(self
            .locked()?
            .1
            .checkpoints
            .get(stream)
            .cloned()
            .unwrap_or_default())
    }

    pub fn applied(
        &self,
        stream: &str,
        versions: Vec<Version>,
        fen: Option<String>,
    ) -> io::Result<()> {
        self.change(|state| {
            let checkpoint = state.checkpoints.entry(stream.to_owned()).or_default();
            checkpoint.versions = versions;
            if fen.is_some() {
                checkpoint.fen = fen;
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(version: &str) -> PendingWrite {
        PendingWrite {
            stream: "moves".into(),
            sender: "alice".into(),
            message: ChessMessage::Move(crate::MovePayload::from_uci("e2e4", "fen", 1, "alice")),
            version: version.into(),
            parent: "0".into(),
        }
    }

    #[test]
    fn restart_preserves_order_versions_and_applied_checkpoint() -> anyhow::Result<()> {
        let root = tempfile::tempdir().context("required operation value")?;
        let journal = Journal::open(root.path(), "backend/game/device")
            .context("required operation value")?;
        journal
            .enqueue(write("one"))
            .context("required operation value")?;
        journal
            .enqueue(write("two"))
            .context("required operation value")?;
        journal
            .reparent("one", "opponent")
            .context("required operation value")?;
        journal
            .enqueue(write("one"))
            .context("required operation value")?;
        journal
            .applied("moves", vec![Version::new("3")], Some("fen".into()))
            .context("required operation value")?;
        drop(journal);
        let restored = Journal::open(root.path(), "backend/game/device")
            .context("required operation value")?;
        let pending = restored.pending().context("required operation value")?;
        assert_eq!(
            pending
                .iter()
                .map(|w| w.version.as_str())
                .collect::<Vec<_>>(),
            ["one", "two"]
        );
        assert_eq!(pending[0].parent, "opponent");
        assert_eq!(
            restored
                .checkpoint("moves")
                .context("required operation value")?
                .fen
                .as_deref(),
            Some("fen")
        );
        restored
            .confirm_write("one")
            .context("required operation value")?;
        assert_eq!(
            restored.pending().context("required operation value")?[0].version,
            "two"
        );
        assert!(Journal::open(root.path(), "other/game/device")
            .context("required operation value")?
            .pending()
            .context("required operation value")?
            .is_empty());
        Ok(())
    }

    #[test]
    fn concurrent_handles_do_not_lose_pending_events_or_hide_corruption() -> anyhow::Result<()> {
        let root = tempfile::tempdir().context("required operation value")?;
        let journal = Journal::open(root.path(), "game").context("required operation value")?;
        std::thread::scope(|scope| -> anyhow::Result<()> {
            let mut jobs = Vec::new();
            for i in 0..8 {
                let journal = journal.clone();
                jobs.push(scope.spawn(move || journal.enqueue(write(&i.to_string()))));
            }
            for job in jobs {
                job.join()
                    .map_err(|_| anyhow::anyhow!("journal writer panicked"))??;
            }
            Ok(())
        })?;
        assert_eq!(
            journal.pending().context("required operation value")?.len(),
            8
        );
        let mut conflict = write("0");
        conflict.sender = "mallory".into();
        assert!(journal.enqueue(conflict).is_err());
        std::fs::write(journal.directory.join("state.json"), "truncated")
            .context("required operation value")?;
        assert!(journal.pending().is_err());
        assert!(journal.enqueue(write("lost")).is_err());
        Ok(())
    }
}
