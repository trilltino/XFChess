// Prevents an extra console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Run native RPC benchmarks and stream results through bench-level.
//! RPC calls run natively, outside browser CORS restrictions.

use std::time::{Duration, Instant};

use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter};

const MAX_CONCURRENCY: u32 = 256;
const MAX_LEVELS: usize = 16;
const MAX_REQUESTS: u32 = 10_000;
static RUNNING: AtomicBool = AtomicBool::new(false);

fn validate_workload(levels: &[u32], requests: u32) -> Result<(), String> {
  if levels.is_empty() || levels.len() > MAX_LEVELS {
    return Err(format!("Choose 1 to {MAX_LEVELS} concurrency levels"));
  }
  if !(1..=MAX_REQUESTS).contains(&requests)
    || levels
      .iter()
      .any(|level| !(1..=MAX_CONCURRENCY).contains(level))
  {
    return Err(format!(
      "Use 1 to {MAX_REQUESTS} requests and concurrency 1 to {MAX_CONCURRENCY}"
    ));
  }
  Ok(())
}

fn worker_requests(total: u32, workers: u32, worker: u32) -> u32 {
  total / workers + u32::from(worker < total % workers)
}

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

fn rpc_succeeded(body: &[u8]) -> bool {
  serde_json::from_slice::<serde_json::Value>(body)
    .is_ok_and(|message| message.get("result").is_some() && message.get("error").is_none())
}

/// Benchmark one (endpoint, concurrency) point with `total` requests split across
/// `concurrency` workers sharing the connection pool.
async fn bench_level(
  client: reqwest::Client,
  endpoint: String,
  url: String,
  concurrency: u32,
  total: u32,
) -> Result<LevelResult, String> {
  let workers = concurrency.min(total);
  let mut set = tokio::task::JoinSet::new();
  let wall = Instant::now();

  for worker in 0..workers {
    let client = client.clone();
    let url = url.clone();
    set.spawn(async move {
      let mut latencies: Vec<f64> = Vec::new();
      let mut throttled = 0u32;
      let mut errors = 0u32;
      let mut body = Vec::with_capacity(1024);
      for request in 0..worker_requests(total, workers, worker) {
        let method = METHODS[((worker + request) as usize) % METHODS.len()];
        let payload = serde_json::json!({ "jsonrpc": "2.0", "id": 1, "method": method });
        let start = Instant::now();
        match client.post(&url).json(&payload).send().await {
          Ok(mut resp) => {
            let status = resp.status();
            body.clear();
            let drained = loop {
              match resp.chunk().await {
                Ok(Some(chunk)) => {
                  if chunk.len() > (1024 * 1024) - body.len() {
                    break false;
                  }
                  body.extend_from_slice(&chunk);
                }
                Ok(None) => break true,
                Err(_) => break false,
              }
            };
            if !drained {
              errors += 1;
              continue;
            }
            let ms = start.elapsed().as_secs_f64() * 1000.0;
            if status.as_u16() == 429 {
              throttled += 1;
            } else if status.is_success() && rpc_succeeded(&body) {
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
    match res {
      Ok((samples, limited, failed)) => {
        latencies.extend(samples);
        throttled += limited;
        errors += failed;
      }
      Err(error) => return Err(format!("Benchmark worker failed: {error}")),
    }
  }

  let secs = wall.elapsed().as_secs_f64().max(1e-6);
  latencies.sort_by(f64::total_cmp);

  Ok(LevelResult {
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
  })
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
  validate_workload(&levels, requests)?;
  for url in [&triton_url, &baseline_url] {
    let url = reqwest::Url::parse(url).map_err(|_| "Invalid RPC endpoint URL")?;
    if !matches!(url.scheme(), "https" | "http")
      || url.host_str().is_none()
      || !url.username().is_empty()
      || url.password().is_some()
    {
      return Err("RPC endpoints must be HTTP(S) URLs without embedded credentials".into());
    }
  }
  RUNNING
    .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
    .map_err(|_| "A benchmark is already running")?;
  struct RunGuard;
  impl Drop for RunGuard {
    fn drop(&mut self) {
      RUNNING.store(false, Ordering::Release);
    }
  }
  let _run = RunGuard;
  let client = reqwest::Client::builder()
    .timeout(Duration::from_secs(20))
    .build()
    .map_err(|e| e.to_string())?;

  for &conc in &levels {
    // Run sequentially per endpoint so the two don't contend for the network.
    let t = bench_level(
      client.clone(),
      "triton".into(),
      triton_url.clone(),
      conc,
      requests,
    )
    .await?;
    let _ = app.emit("bench-level", &t);
    let b = bench_level(
      client.clone(),
      "baseline".into(),
      baseline_url.clone(),
      conc,
      requests,
    )
    .await?;
    let _ = app.emit("bench-level", &b);
  }

  let _ = app.emit("bench-done", ());
  Ok(())
}

fn main() -> Result<(), tauri::Error> {
  tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![run_read_load])
    .run(tauri::generate_context!())?;
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn rpc_errors_and_malformed_bodies_do_not_count_as_success() {
    assert!(rpc_succeeded(br#"{"jsonrpc":"2.0","result":42,"id":1}"#));
    assert!(!rpc_succeeded(
      br#"{"jsonrpc":"2.0","error":{"code":-32601},"id":1}"#
    ));
    assert!(!rpc_succeeded(br#"{"jsonrpc":"2.0","id":1}"#));
    assert!(!rpc_succeeded(b"not JSON"));
  }

  #[test]
  fn workload_limits_reject_invalid_native_inputs() {
    assert!(validate_workload(&[1, 8, 32, 64], 120).is_ok());
    assert!(validate_workload(&[], 1).is_err());
    assert!(validate_workload(&[1; MAX_LEVELS + 1], 1).is_err());
    for (levels, total) in [
      (vec![0], 1),
      (vec![257], 1),
      (vec![1], 0),
      (vec![1], 10_001),
    ] {
      assert!(validate_workload(&levels, total).is_err());
    }
  }

  #[test]
  fn workers_issue_exactly_the_requested_count() {
    for total in [1, 7, 120, MAX_REQUESTS] {
      for concurrency in [1, 8, 64, MAX_CONCURRENCY] {
        let workers = concurrency.min(total);
        assert_eq!(
          (0..workers)
            .map(|worker| worker_requests(total, workers, worker))
            .sum::<u32>(),
          total
        );
      }
    }
  }
}
