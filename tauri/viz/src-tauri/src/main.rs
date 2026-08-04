// Prevents an extra console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! XFChess Network Visualiser — native backend.
//!
//! Runs the read-RPC load benchmark in Rust (no browser CORS, token stays native)
//! and streams per-level results to the webview via the `bench-level` event.

use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

const METHODS: [&str; 5] = [
    "getSlot",
    "getLatestBlockhash",
    "getVersion",
    "getHealth",
    "getBlockHeight",
];

/// One (endpoint, concurrency) measurement, emitted on `bench-level`.
#[derive(Serialize, Clone)]
struct LevelResult {
    endpoint: String, // "triton" | "baseline"
    concurrency: u32,
    ok: u32,
    p50: f64,
    p95: f64,
    p99: f64,
    max: f64,
    throttled: u32,
    errors: u32,
    rps: f64,
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let rank = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
    sorted[rank.min(sorted.len() - 1)]
}

/// Benchmark one (endpoint, concurrency) point with `total` requests split across
/// `concurrency` workers sharing the connection pool.
async fn bench_level(
    client: reqwest::Client,
    endpoint: String,
    url: String,
    concurrency: u32,
    total: u32,
) -> LevelResult {
    let per_worker = total.div_ceil(concurrency.max(1));
    let mut set = tokio::task::JoinSet::new();
    let wall = Instant::now();

    for w in 0..concurrency {
        let client = client.clone();
        let url = url.clone();
        set.spawn(async move {
            let mut latencies: Vec<f64> = Vec::new();
            let mut throttled = 0u32;
            let mut errors = 0u32;
            for i in 0..per_worker {
                let method = METHODS[((w * per_worker + i) as usize) % METHODS.len()];
                let body = serde_json::json!({ "jsonrpc": "2.0", "id": 1, "method": method });
                let start = Instant::now();
                match client.post(&url).json(&body).send().await {
                    Ok(resp) => {
                        let status = resp.status();
                        let _ = resp.bytes().await; // drain so the connection is reused
                        let ms = start.elapsed().as_secs_f64() * 1000.0;
                        if status.as_u16() == 429 {
                            throttled += 1;
                        } else if status.is_success() {
                            latencies.push(ms);
                        } else {
                            errors += 1;
                        }
                    }
                    Err(_) => errors += 1,
                }
            }
            (latencies, throttled, errors)
        });
    }

    let mut latencies: Vec<f64> = Vec::new();
    let mut throttled = 0u32;
    let mut errors = 0u32;
    while let Some(res) = set.join_next().await {
        if let Ok((l, t, e)) = res {
            latencies.extend(l);
            throttled += t;
            errors += e;
        }
    }

    let secs = wall.elapsed().as_secs_f64().max(1e-6);
    latencies.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    LevelResult {
        endpoint,
        concurrency,
        ok: latencies.len() as u32,
        p50: percentile(&latencies, 50.0),
        p95: percentile(&latencies, 95.0),
        p99: percentile(&latencies, 99.0),
        max: latencies.last().copied().unwrap_or(0.0),
        throttled,
        errors,
        rps: latencies.len() as f64 / secs,
    }
}

/// Run the full ramp for both endpoints, emitting each level as it completes.
#[tauri::command]
async fn run_read_load(
    app: AppHandle,
    triton_url: String,
    baseline_url: String,
    levels: Vec<u32>,
    requests: u32,
) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;

    for &conc in &levels {
        // Run sequentially per endpoint so the two don't contend for the network.
        let t = bench_level(client.clone(), "triton".into(), triton_url.clone(), conc, requests).await;
        let _ = app.emit("bench-level", &t);
        let b = bench_level(client.clone(), "baseline".into(), baseline_url.clone(), conc, requests).await;
        let _ = app.emit("bench-level", &b);
    }

    let _ = app.emit("bench-done", ());
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![run_read_load])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
