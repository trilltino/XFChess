//! Triton vs. baseline RPC benchmark suite.
//!
//! Three independent probes that validate the "Part 1" claims for the project:
//!   * [`read_load`]   — read-RPC latency + 429 rate under ramping concurrency.
//!   * [`tx_land`]     — transaction submit/confirm timing (landing reliability).
//!   * [`geyser`]      — Yellowstone gRPC push-streaming connectivity (feature-gated).
//!
//! None of these change on-chain CU costs (those are deterministic) — they measure
//! the *infrastructure* difference between a dedicated endpoint and shared public RPC.

pub mod read_load;
pub mod stream;
pub mod tx_land;

#[cfg(feature = "geyser")]
pub mod geyser;

/// A collection of latency samples (milliseconds) plus error/throttle counts.
#[derive(Default, Clone)]
pub struct LatencyStats {
    /// Successful-request latencies, in milliseconds.
    pub samples: Vec<f64>,
    /// Non-2xx / transport errors that were not HTTP 429.
    pub errors: u64,
    /// HTTP 429 (Too Many Requests) responses — the rate-limit signal.
    pub throttled: u64,
}

impl LatencyStats {
    pub fn record_ms(&mut self, ms: f64) {
        self.samples.push(ms);
    }

    pub fn record_error(&mut self) {
        self.errors += 1;
    }

    pub fn record_throttle(&mut self) {
        self.throttled += 1;
    }

    /// Fold another stats bucket into this one.
    pub fn merge(&mut self, other: LatencyStats) {
        self.samples.extend(other.samples);
        self.errors += other.errors;
        self.throttled += other.throttled;
    }

    pub fn ok(&self) -> usize {
        self.samples.len()
    }

    /// Total attempted requests (ok + errors + throttled).
    pub fn total(&self) -> u64 {
        self.samples.len() as u64 + self.errors + self.throttled
    }

    pub fn mean(&self) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }
        self.samples.iter().sum::<f64>() / self.samples.len() as f64
    }

    pub fn min(&self) -> f64 {
        self.samples
            .iter()
            .cloned()
            .fold(f64::INFINITY, f64::min)
            .max(0.0)
    }

    pub fn max(&self) -> f64 {
        self.samples.iter().cloned().fold(0.0, f64::max)
    }

    /// Nearest-rank percentile (p in 0..=100) over the recorded latencies.
    pub fn percentile(&self, p: f64) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let rank = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
        sorted[rank.min(sorted.len() - 1)]
    }
}

/// Hide a token that lives in the URL path or query so benchmark output is safe to
/// paste/screenshot. Masks the last `/`-segment if it looks like a secret.
pub fn redact_url(url: &str) -> String {
    if let Some(idx) = url.rfind('/') {
        let (head, tail) = url.split_at(idx + 1);
        if tail.len() >= 16 {
            return format!("{head}***");
        }
    }
    url.to_string()
}
