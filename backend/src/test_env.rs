//! Serializes tests that mutate the process environment.
use tokio::sync::{Mutex, MutexGuard};

static ENV: Mutex<()> = Mutex::const_new(());

pub async fn lock() -> MutexGuard<'static, ()> {
    ENV.lock().await
}
