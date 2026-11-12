use serde::Serialize;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use sysinfo::System;

#[derive(Clone, Serialize)]
pub struct HostResources {
    pub cpu_percent: Option<f32>,
    pub memory_used_bytes: u64,
    pub memory_total_bytes: u64,
    pub observed_at: u64,
    pub source: &'static str,
}

#[derive(Default)]
struct Sampler {
    system: System,
    sampled_at: Option<Instant>,
    cached: Option<HostResources>,
}

pub fn snapshot() -> Option<HostResources> {
    static SAMPLER: OnceLock<Mutex<Sampler>> = OnceLock::new();
    let mut sampler = SAMPLER
        .get_or_init(|| Mutex::new(Sampler::default()))
        .lock()
        .ok()?;
    if sampler
        .sampled_at
        .is_some_and(|at| at.elapsed() < Duration::from_secs(1))
    {
        return sampler.cached.clone();
    }
    sampler.system.refresh_cpu_usage();
    sampler.system.refresh_memory();
    let result = HostResources {
        // CPU is a delta between samples; the first observation has no valid interval.
        cpu_percent: sampler
            .sampled_at
            .map(|_| sampler.system.global_cpu_usage().clamp(0.0, 100.0)),
        memory_used_bytes: sampler.system.used_memory(),
        memory_total_bytes: sampler.system.total_memory(),
        observed_at: SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs(),
        source: "backend_host",
    };
    sampler.sampled_at = Some(Instant::now());
    sampler.cached = Some(result.clone());
    Some(result)
}
