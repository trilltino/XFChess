//! Read-RPC load test: fire light JSON-RPC reads at ramping concurrency and
//! report latency percentiles + the HTTP 429 (throttle) rate per endpoint.
//!
//! This is the head-to-head that proves the "kills the 429s" claim: run it against
//! the Triton endpoint and against public devnet and compare the `429` columns.

use std::sync::Arc;
use std::time::{Duration, Instant};

use reqwest::Client;
use serde_json::{json, Value};
use tokio::task::JoinSet;

use super::{redact_url, LatencyStats};

/// A named RPC endpoint to benchmark.
pub struct Target {
    pub name: String,
    pub url: String,
}

/// Lightweight, side-effect-free read methods rotated across requests.
const METHODS: &[&str] = &[
    "getSlot",
    "getLatestBlockhash",
    "getVersion",
    "getHealth",
    "getBlockHeight",
];

fn method_body(i: usize) -> Value {
    let method = METHODS[i % METHODS.len()];
    json!({ "jsonrpc": "2.0", "id": 1, "method": method })
}

/// Run the full ramp for every target and print a comparison table each.
pub async fn run(targets: &[Target], levels: &[usize], requests_per_level: usize) {
    println!("\n╔══════════════════════════════════════════════════════════╗");
    println!("║  READ-RPC LOAD TEST  (latency + 429 rate under concurrency)  ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!(
        "   {} requests per level · methods: {}",
        requests_per_level,
        METHODS.join(", ")
    );

    // grid[target][level] = (stats, requests-per-second)
    let mut grid: Vec<Vec<(LatencyStats, f64)>> = Vec::with_capacity(targets.len());

    for target in targets {
        println!("\n── {} ──  {}", target.name, redact_url(&target.url));
        println!(
            "   {:>5} │ {:>6} │ {:>8} │ {:>8} │ {:>8} │ {:>8} │ {:>6} │ {:>6} │ {:>9}",
            "conc", "ok", "p50(ms)", "p95(ms)", "p99(ms)", "max(ms)", "429", "err", "req/s"
        );
        println!("   {}", "─".repeat(82));

        let mut row = Vec::with_capacity(levels.len());
        for &conc in levels {
            let wall = Instant::now();
            let stats = bench_level(&target.url, conc, requests_per_level).await;
            let elapsed = wall.elapsed().as_secs_f64().max(1e-6);
            let rps = stats.ok() as f64 / elapsed;
            println!(
                "   {:>5} │ {:>6} │ {:>8.1} │ {:>8.1} │ {:>8.1} │ {:>8.1} │ {:>6} │ {:>6} │ {:>9.1}",
                conc,
                stats.ok(),
                stats.percentile(50.0),
                stats.percentile(95.0),
                stats.percentile(99.0),
                stats.max(),
                stats.throttled,
                stats.errors,
                rps,
            );
            row.push((stats, rps));
        }
        grid.push(row);
    }

    render_charts(targets, levels, &grid);
    println!("\n   Read 429>0 on public devnet but 429≈0 on Triton = the throttling win is real.");
}

/// Render horizontal bar charts so the Triton-vs-baseline gap is visible at a glance.
fn render_charts(targets: &[Target], levels: &[usize], grid: &[Vec<(LatencyStats, f64)>]) {
    const WIDTH: usize = 40;
    let label_w = targets
        .iter()
        .map(|t| t.name.len())
        .max()
        .unwrap_or(8)
        .min(16);

    // ── Throughput chart ──────────────────────────────────────────────────────
    let max_rps = grid
        .iter()
        .flatten()
        .map(|(_, rps)| *rps)
        .fold(0.0_f64, f64::max)
        .max(1.0);

    println!("\n   ┌─ Successful throughput (req/s) ─ higher is better ─────────────────");
    for (li, &conc) in levels.iter().enumerate() {
        println!("   │ concurrency {conc}");
        for (ti, target) in targets.iter().enumerate() {
            let (_, rps) = &grid[ti][li];
            println!(
                "   │   {:<lw$} {} {:.0}",
                truncate(&target.name, label_w),
                bar(*rps, max_rps, WIDTH),
                rps,
                lw = label_w,
            );
        }
    }

    // ── Throttle chart ────────────────────────────────────────────────────────
    println!("   ├─ Throttled requests (% HTTP 429) ─ lower is better ───────────────");
    for (li, &conc) in levels.iter().enumerate() {
        println!("   │ concurrency {conc}");
        for (ti, target) in targets.iter().enumerate() {
            let (stats, _) = &grid[ti][li];
            let total = stats.total().max(1);
            let pct = stats.throttled as f64 / total as f64 * 100.0;
            println!(
                "   │   {:<lw$} {} {:.0}%",
                truncate(&target.name, label_w),
                bar(pct, 100.0, WIDTH),
                pct,
                lw = label_w,
            );
        }
    }
    println!("   └────────────────────────────────────────────────────────────────────");
}

/// A filled horizontal bar scaled to `max` over `width` columns.
fn bar(value: f64, max: f64, width: usize) -> String {
    if max <= 0.0 {
        return String::new();
    }
    let filled = ((value / max) * width as f64).round() as usize;
    let filled = filled.min(width);
    format!("{}{}", "█".repeat(filled), "·".repeat(width - filled))
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        s[..max].to_string()
    }
}

/// Benchmark one (endpoint, concurrency) point with `total` requests split evenly
/// across `concurrency` workers sharing a connection pool.
async fn bench_level(url: &str, concurrency: usize, total: usize) -> LatencyStats {
    let client = Arc::new(
        Client::builder()
            .pool_max_idle_per_host(concurrency)
            .timeout(Duration::from_secs(20))
            .build()
            .expect("build reqwest client"),
    );

    let per_worker = total.div_ceil(concurrency);
    let mut set: JoinSet<LatencyStats> = JoinSet::new();

    for w in 0..concurrency {
        let client = Arc::clone(&client);
        let url = url.to_string();
        set.spawn(async move {
            let mut local = LatencyStats::default();
            for i in 0..per_worker {
                let body = method_body(w * per_worker + i);
                let start = Instant::now();
                match client.post(&url).json(&body).send().await {
                    Ok(resp) => {
                        let status = resp.status();
                        // Drain the body so the connection can be reused.
                        let _ = resp.bytes().await;
                        let ms = start.elapsed().as_secs_f64() * 1000.0;
                        if status.as_u16() == 429 {
                            local.record_throttle();
                        } else if status.is_success() {
                            local.record_ms(ms);
                        } else {
                            local.record_error();
                        }
                    }
                    Err(_) => local.record_error(),
                }
            }
            local
        });
    }

    let mut agg = LatencyStats::default();
    while let Some(res) = set.join_next().await {
        if let Ok(local) = res {
            agg.merge(local);
        }
    }
    agg
}
