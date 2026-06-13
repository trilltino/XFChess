//! Static counters exposed by the analysis pipeline.
//!
//! The backend renders these onto its `/metrics` endpoint; keeping them as
//! plain atomics in this crate lets the worker loop update them without a
//! dependency on the backend's telemetry types.

use std::sync::atomic::AtomicU64;

/// Completed Stockfish analyses.
pub static ANALYSES_TOTAL: AtomicU64 = AtomicU64::new(0);
/// Analyses that errored (after retries these games age out of the queue).
pub static ANALYSIS_FAILURES_TOTAL: AtomicU64 = AtomicU64::new(0);
/// Wall-clock duration of the most recent analysis, in milliseconds.
pub static ANALYSIS_MILLIS_LAST: AtomicU64 = AtomicU64::new(0);
