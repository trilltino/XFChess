use std::sync::atomic::AtomicU64;

pub static ANALYSES_TOTAL: AtomicU64 = AtomicU64::new(0);
pub static ANALYSIS_FAILURES_TOTAL: AtomicU64 = AtomicU64::new(0);
pub static ANALYSIS_MILLIS_LAST: AtomicU64 = AtomicU64::new(0);
