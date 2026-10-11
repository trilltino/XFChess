//! Serialises tests that change process environment variables.
//!
//! Tests in one binary run on parallel threads but share one environment, so a
//! test that sets `ADMIN_API_KEY` can race another that removes it. Every such
//! test holds [`lock`] for its whole body, across modules.

use std::sync::{Mutex, MutexGuard};

static ENV: Mutex<()> = Mutex::new(());

/// Holds the environment for the caller. A test that panicked while holding it
/// doesn't poison the rest.
pub fn lock() -> MutexGuard<'static, ()> {
    ENV.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}
