//! Prometheus-compatible metrics for XFChess backend
//!
//! Tracks HTTP performance, Solana transactions/RPC, and fee-payer health.
//! `AppState.metrics: Arc<Metrics>` is shared everywhere, so every field uses
//! interior mutability (a lock around the map only for inserting a new key,
//! atomics for the counts themselves) — recording never needs `&mut self`.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;

/// Upper bounds (inclusive, milliseconds) of the fixed duration-histogram
/// buckets shared by HTTP and Solana RPC/transaction timings. Prometheus
/// histograms are cumulative ("le" = less-or-equal); the `+Inf` bucket is
/// implicit and equals the total count.
const DURATION_BUCKETS_MS: [f64; 9] = [5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2500.0];

#[derive(Debug)]
struct DurationHistogram {
    bucket_counts: [AtomicU64; DURATION_BUCKETS_MS.len()],
    sum_ms: AtomicU64,
    count: AtomicU64,
}

impl DurationHistogram {
    fn new() -> Self {
        Self {
            bucket_counts: std::array::from_fn(|_| AtomicU64::new(0)),
            sum_ms: AtomicU64::new(0),
            count: AtomicU64::new(0),
        }
    }

    fn record(&self, duration_ms: f64) {
        let duration_ms = duration_ms.max(0.0);
        for (i, bound) in DURATION_BUCKETS_MS.iter().enumerate() {
            if duration_ms <= *bound {
                self.bucket_counts[i].fetch_add(1, Ordering::Relaxed);
            }
        }
        self.sum_ms.fetch_add(duration_ms.round() as u64, Ordering::Relaxed);
        self.count.fetch_add(1, Ordering::Relaxed);
    }

    /// Appends this histogram's series (bucket/sum/count lines) to `out`,
    /// under Prometheus metric name `name` with the given label set.
    fn write_prometheus(&self, out: &mut String, name: &str, labels: &str) {
        let total = self.count.load(Ordering::Relaxed);
        for (i, bound) in DURATION_BUCKETS_MS.iter().enumerate() {
            let cumulative = self.bucket_counts[i].load(Ordering::Relaxed);
            out.push_str(&format!(
                "{name}_bucket{{{labels},le=\"{bound}\"}} {cumulative}\n"
            ));
        }
        out.push_str(&format!("{name}_bucket{{{labels},le=\"+Inf\"}} {total}\n"));
        out.push_str(&format!(
            "{name}_sum{{{labels}}} {}\n",
            self.sum_ms.load(Ordering::Relaxed)
        ));
        out.push_str(&format!("{name}_count{{{labels}}} {total}\n"));
    }
}

/// Increments the `AtomicU64` counter at `key` in `map`, inserting it first if
/// this is the first time this key has been recorded. Read-locked in the
/// common (key-already-exists) case; only takes the write lock to insert.
fn bump<K: std::hash::Hash + Eq + Clone>(map: &RwLock<HashMap<K, AtomicU64>>, key: &K) {
    if let Some(counter) = map.read().unwrap().get(key) {
        counter.fetch_add(1, Ordering::Relaxed);
        return;
    }
    map.write()
        .unwrap()
        .entry(key.clone())
        .or_insert_with(|| AtomicU64::new(0))
        .fetch_add(1, Ordering::Relaxed);
}

/// Records `duration_ms` into the histogram at `key` in `map`, inserting a
/// fresh histogram first if this is the first observation for that key.
fn observe<K: std::hash::Hash + Eq + Clone>(
    map: &RwLock<HashMap<K, DurationHistogram>>,
    key: &K,
    duration_ms: f64,
) {
    if let Some(hist) = map.read().unwrap().get(key) {
        hist.record(duration_ms);
        return;
    }
    map.write()
        .unwrap()
        .entry(key.clone())
        .or_insert_with(DurationHistogram::new)
        .record(duration_ms);
}

/// Core metrics collection. Shared as `Arc<Metrics>` — all methods take `&self`.
#[derive(Debug, Default)]
pub struct Metrics {
    // API metrics
    http_requests_total: RwLock<HashMap<(String, u16), AtomicU64>>,
    http_request_duration_ms: RwLock<HashMap<String, DurationHistogram>>,

    // Solana RPC metrics
    solana_rpc_calls_total: RwLock<HashMap<(String, String), AtomicU64>>,
    solana_rpc_latency_ms: RwLock<HashMap<String, DurationHistogram>>,

    // Transaction metrics
    transactions_submitted_total: RwLock<HashMap<String, AtomicU64>>,
    transactions_confirmed_total: RwLock<HashMap<String, AtomicU64>>,
    transactions_failed_total: RwLock<HashMap<(String, String), AtomicU64>>,
    transaction_confirmation_ms: RwLock<HashMap<String, DurationHistogram>>,

    // Fee payer metrics
    feepayer_balance_lamports: RwLock<HashMap<usize, AtomicU64>>,
}

impl Metrics {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record an HTTP request (endpoint = "METHOD /path").
    pub fn record_http_request(&self, endpoint: &str, status: u16, duration_ms: f64) {
        bump(&self.http_requests_total, &(endpoint.to_string(), status));
        observe(&self.http_request_duration_ms, &endpoint.to_string(), duration_ms);
    }

    /// Record a Solana RPC call (e.g. `getSlot`, `getMultipleAccounts`).
    pub fn record_solana_rpc_call(&self, method: &str, success: bool, latency_ms: f64) {
        let status = if success { "success" } else { "error" };
        bump(
            &self.solana_rpc_calls_total,
            &(method.to_string(), status.to_string()),
        );
        observe(&self.solana_rpc_latency_ms, &method.to_string(), latency_ms);
    }

    /// Record transaction submission (fired before confirmation is known).
    pub fn record_transaction_submitted(&self, chain_type: &str) {
        bump(&self.transactions_submitted_total, &chain_type.to_string());
    }

    /// Record transaction confirmation.
    pub fn record_transaction_confirmed(&self, chain_type: &str, confirmation_time_ms: f64) {
        bump(&self.transactions_confirmed_total, &chain_type.to_string());
        observe(
            &self.transaction_confirmation_ms,
            &chain_type.to_string(),
            confirmation_time_ms,
        );
    }

    /// Record transaction failure, classified by `error_type` (see
    /// `signing::solana::telemetry::TxErrorCategory`).
    pub fn record_transaction_failed(&self, chain_type: &str, error_type: &str) {
        bump(
            &self.transactions_failed_total,
            &(chain_type.to_string(), error_type.to_string()),
        );
    }

    /// Update fee payer balance (called from the `/health/detailed` fee-payer
    /// check, keyed by fee-payer pool index).
    pub fn update_feepayer_balance(&self, key_index: usize, lamports: u64) {
        if let Some(gauge) = self.feepayer_balance_lamports.read().unwrap().get(&key_index) {
            gauge.store(lamports, Ordering::Relaxed);
            return;
        }
        self.feepayer_balance_lamports
            .write()
            .unwrap()
            .entry(key_index)
            .or_insert_with(|| AtomicU64::new(0))
            .store(lamports, Ordering::Relaxed);
    }

    /// Current confirmed-transaction count for `chain_type` ("solana"/"er").
    /// Used by `GET /admin/metrics/summary` — a plain JSON read of the same
    /// data `/metrics` exports, for the admin panel's KPI tile.
    pub fn transactions_confirmed(&self, chain_type: &str) -> u64 {
        self.transactions_confirmed_total
            .read()
            .unwrap()
            .get(chain_type)
            .map(|c| c.load(Ordering::Relaxed))
            .unwrap_or(0)
    }

    /// Current fee-payer balance gauge for `key_index` (see `update_feepayer_balance`).
    pub fn feepayer_balance(&self, key_index: usize) -> u64 {
        self.feepayer_balance_lamports
            .read()
            .unwrap()
            .get(&key_index)
            .map(|g| g.load(Ordering::Relaxed))
            .unwrap_or(0)
    }

    /// Export metrics in Prometheus text format.
    pub fn export_prometheus_format(&self) -> String {
        let mut output = String::new();

        output.push_str("# HELP http_requests_total Total HTTP requests\n");
        output.push_str("# TYPE http_requests_total counter\n");
        for ((endpoint, status), count) in self.http_requests_total.read().unwrap().iter() {
            let value = count.load(Ordering::Relaxed);
            output.push_str(&format!(
                "http_requests_total{{endpoint=\"{}\",status=\"{}\"}} {}\n",
                endpoint, status, value
            ));
        }

        output.push_str("\n# HELP http_request_duration_ms HTTP request duration in milliseconds\n");
        output.push_str("# TYPE http_request_duration_ms histogram\n");
        for (endpoint, hist) in self.http_request_duration_ms.read().unwrap().iter() {
            hist.write_prometheus(
                &mut output,
                "http_request_duration_ms",
                &format!("endpoint=\"{endpoint}\""),
            );
        }

        output.push_str("\n# HELP solana_rpc_calls_total Total Solana RPC calls\n");
        output.push_str("# TYPE solana_rpc_calls_total counter\n");
        for ((method, status), count) in self.solana_rpc_calls_total.read().unwrap().iter() {
            output.push_str(&format!(
                "solana_rpc_calls_total{{method=\"{}\",status=\"{}\"}} {}\n",
                method,
                status,
                count.load(Ordering::Relaxed)
            ));
        }

        output.push_str("\n# HELP solana_rpc_latency_ms Solana RPC latency in milliseconds\n");
        output.push_str("# TYPE solana_rpc_latency_ms histogram\n");
        for (method, hist) in self.solana_rpc_latency_ms.read().unwrap().iter() {
            hist.write_prometheus(
                &mut output,
                "solana_rpc_latency_ms",
                &format!("method=\"{method}\""),
            );
        }

        output.push_str("\n# HELP transactions_submitted_total Total transactions submitted\n");
        output.push_str("# TYPE transactions_submitted_total counter\n");
        for (chain_type, count) in self.transactions_submitted_total.read().unwrap().iter() {
            output.push_str(&format!(
                "transactions_submitted_total{{chain=\"{}\"}} {}\n",
                chain_type,
                count.load(Ordering::Relaxed)
            ));
        }

        output.push_str("\n# HELP transactions_confirmed_total Total transactions confirmed\n");
        output.push_str("# TYPE transactions_confirmed_total counter\n");
        for (chain_type, count) in self.transactions_confirmed_total.read().unwrap().iter() {
            output.push_str(&format!(
                "transactions_confirmed_total{{chain=\"{}\"}} {}\n",
                chain_type,
                count.load(Ordering::Relaxed)
            ));
        }

        output.push_str("\n# HELP transactions_failed_total Total transactions failed\n");
        output.push_str("# TYPE transactions_failed_total counter\n");
        for ((chain_type, error_type), count) in self.transactions_failed_total.read().unwrap().iter() {
            output.push_str(&format!(
                "transactions_failed_total{{chain=\"{}\",error_type=\"{}\"}} {}\n",
                chain_type,
                error_type,
                count.load(Ordering::Relaxed)
            ));
        }

        output.push_str("\n# HELP transaction_confirmation_ms Transaction confirmation time in milliseconds\n");
        output.push_str("# TYPE transaction_confirmation_ms histogram\n");
        for (chain_type, hist) in self.transaction_confirmation_ms.read().unwrap().iter() {
            hist.write_prometheus(
                &mut output,
                "transaction_confirmation_ms",
                &format!("chain=\"{chain_type}\""),
            );
        }

        output.push_str("\n# HELP feepayer_balance_lamports Fee payer balance in lamports\n");
        output.push_str("# TYPE feepayer_balance_lamports gauge\n");
        for (key_index, gauge) in self.feepayer_balance_lamports.read().unwrap().iter() {
            output.push_str(&format!(
                "feepayer_balance_lamports{{key_index=\"{}\"}} {}\n",
                key_index,
                gauge.load(Ordering::Relaxed)
            ));
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_request_counts_and_histogram_accumulate() {
        let m = Metrics::new();
        m.record_http_request("GET /health", 200, 3.0);
        m.record_http_request("GET /health", 200, 12.0);
        m.record_http_request("GET /health", 500, 4000.0);

        let out = m.export_prometheus_format();
        assert!(out.contains("http_requests_total{endpoint=\"GET /health\",status=\"200\"} 2"));
        assert!(out.contains("http_requests_total{endpoint=\"GET /health\",status=\"500\"} 1"));
        // 3ms and 12ms both land at-or-under the 25ms bucket; the 4000ms one only
        // shows up in +Inf.
        assert!(out.contains("http_request_duration_ms_bucket{endpoint=\"GET /health\",le=\"25\"} 2"));
        assert!(out.contains("http_request_duration_ms_bucket{endpoint=\"GET /health\",le=\"+Inf\"} 3"));
        assert!(out.contains("http_request_duration_ms_count{endpoint=\"GET /health\"} 3"));
    }

    #[test]
    fn transaction_lifecycle_counters() {
        let m = Metrics::new();
        m.record_transaction_submitted("solana");
        m.record_transaction_confirmed("solana", 850.0);
        m.record_transaction_failed("solana", "RpcTimeout");

        let out = m.export_prometheus_format();
        assert!(out.contains("transactions_submitted_total{chain=\"solana\"} 1"));
        assert!(out.contains("transactions_confirmed_total{chain=\"solana\"} 1"));
        assert!(out.contains("transactions_failed_total{chain=\"solana\",error_type=\"RpcTimeout\"} 1"));
    }

    #[test]
    fn feepayer_balance_is_a_gauge_not_a_counter() {
        let m = Metrics::new();
        m.update_feepayer_balance(0, 5_000_000_000);
        m.update_feepayer_balance(0, 4_500_000_000);
        let out = m.export_prometheus_format();
        assert!(out.contains("feepayer_balance_lamports{key_index=\"0\"} 4500000000"));
    }
}
