//! Background work that must never stall a frame.
//!
//! The client's HTTP calls use blocking `reqwest` (async `reqwest` needs a Tokio
//! reactor, which Bevy's task pools do not provide). Blocking work therefore runs
//! on its own short-lived thread, never inside `IoTaskPool` (whose few workers it
//! would park for the whole request):
//!
//! - [`spawn_detached`] — fire-and-forget (posting telemetry, best-effort syncs).
//! - [`Pending`] — work whose result a system polls each frame.
//! - [`take_finished`] — the same polling for async work already running on the
//!   multiplayer Tokio runtime (`tokio::task::JoinHandle`).

use crossbeam_channel::{bounded, Receiver, TryRecvError};

/// Runs blocking `work` on a named background thread and forgets about it.
pub fn spawn_detached(name: &'static str, work: impl FnOnce() + Send + 'static) {
    if let Err(e) = std::thread::Builder::new()
        .name(format!("xf-{name}"))
        .spawn(work)
    {
        bevy::log::error!("[net_task] could not start background thread {name}: {e}");
    }
}

/// The result of blocking work running on a background thread.
///
/// Keep it in a resource or component and call [`Pending::poll`] from a system;
/// dropping it simply discards the result when the work finishes.
pub struct Pending<T> {
    rx: Receiver<T>,
}

/// What [`Pending::poll`] saw this frame.
#[derive(Debug, PartialEq, Eq)]
pub enum Poll<T> {
    /// Still running.
    Running,
    Ready(T),
    /// The worker died without producing a result (it panicked).
    Lost,
}

impl<T: Send + 'static> Pending<T> {
    pub fn spawn(name: &'static str, work: impl FnOnce() -> T + Send + 'static) -> Self {
        let (tx, rx) = bounded(1);
        spawn_detached(name, move || {
            let _ = tx.send(work());
        });
        Self { rx }
    }

    pub fn poll(&self) -> Poll<T> {
        match self.rx.try_recv() {
            Ok(value) => Poll::Ready(value),
            Err(TryRecvError::Empty) => Poll::Running,
            Err(TryRecvError::Disconnected) => Poll::Lost,
        }
    }
}

/// Takes the result out of `slot` once its Tokio task has finished; `None` while it
/// is still running or when nothing is pending. A panicked or cancelled task becomes
/// `Err`.
pub fn take_finished<T>(
    slot: &mut Option<tokio::task::JoinHandle<Result<T, String>>>,
) -> Option<Result<T, String>> {
    if !slot.as_ref()?.is_finished() {
        return None;
    }
    let handle = slot.take()?;
    // The task has finished, so this resolves immediately.
    Some(
        futures_lite::future::block_on(handle)
            .unwrap_or_else(|e| Err(format!("background task failed: {e}"))),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn wait<T: Send + 'static>(p: &Pending<T>) -> Poll<T> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match p.poll() {
                Poll::Running if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(5))
                }
                other => return other,
            }
        }
    }

    #[test]
    fn pending_delivers_its_result_once() {
        let p = Pending::spawn("test", || 41 + 1);
        assert_eq!(wait(&p), Poll::Ready(42));
        assert_eq!(
            p.poll(),
            Poll::Lost,
            "nothing left after the result is taken"
        );
    }

    #[test]
    fn a_panicking_worker_is_reported_lost() {
        let p: Pending<u8> = Pending::spawn("test-panic", || panic!("boom"));
        assert_eq!(wait(&p), Poll::Lost);
    }

    #[test]
    fn take_finished_waits_for_the_task() -> Result<(), Box<dyn std::error::Error>> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let mut slot = Some(rt.spawn(async { Ok::<_, String>(7) }));
        rt.block_on(async { tokio::task::yield_now().await });
        let deadline = Instant::now() + Duration::from_secs(5);
        let result = loop {
            if let Some(r) = take_finished(&mut slot) {
                break r;
            }
            assert!(Instant::now() < deadline);
            rt.block_on(async { tokio::task::yield_now().await });
        };
        assert_eq!(result, Ok(7));
        assert!(slot.is_none());
        assert_eq!(take_finished::<u8>(&mut None), None);

        Ok(())
    }
}
