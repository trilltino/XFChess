use std::sync::Arc;
use std::time::{Duration, Instant};

use reqwest::Client;
use serde_json::{json, Value};
use tokio::task::JoinSet;

use super::{redact_url, LatencyStats};

pub struct Target {
    pub name: String,
    pub url: String,
}

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

pub async fn run(
    targets: &[Target],
    levels: &[usize],
    requests_per_level: usize,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        requests_per_level > 0 && requests_per_level <= 100_000,
        "request count must be in 1..=100000"
    );
    anyhow::ensure!(
        !levels.is_empty()
            && levels.len() <= 16
            && levels.iter().all(|level| (1..=256).contains(level)),
        "choose 1..=16 concurrency levels in 1..=256"
    );
    println!("\n╔══════════════════════════════════════════════════════════╗");
    println!("║  READ-RPC LOAD TEST  (latency + 429 rate under concurrency)  ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!(
        "   {} requests per level · methods: {}",
        requests_per_level,
        METHODS.join(", ")
    );

    for target in targets {
        println!("\n── {} ──  {}", target.name, redact_url(&target.url));
        println!(
            "   {:>5} │ {:>6} │ {:>8} │ {:>8} │ {:>8} │ {:>8} │ {:>6} │ {:>6} │ {:>9}",
            "conc", "ok", "p50(ms)", "p95(ms)", "p99(ms)", "max(ms)", "429", "err", "req/s"
        );
        println!("   {}", "─".repeat(82));

        for &conc in levels {
            let wall = Instant::now();
            let stats = bench_level(&target.url, conc, requests_per_level).await?;
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
        }
    }
    Ok(())
}

async fn bench_level(url: &str, concurrency: usize, total: usize) -> anyhow::Result<LatencyStats> {
    if concurrency == 0 || total == 0 {
        return Ok(LatencyStats::default());
    }
    let client = Arc::new(
        Client::builder()
            .pool_max_idle_per_host(concurrency)
            .timeout(Duration::from_secs(20))
            .build()?,
    );

    let per_worker = total.div_ceil(concurrency);
    let cooldown = Arc::new(tokio::sync::Mutex::new(tokio::time::Instant::now()));
    let mut set: JoinSet<LatencyStats> = JoinSet::new();

    for w in 0..concurrency.min(total) {
        let client = Arc::clone(&client);
        let cooldown = Arc::clone(&cooldown);
        let url = url.to_string();
        set.spawn(async move {
            let mut local = LatencyStats::default();
            for i in 0..per_worker {
                let request_index = w * per_worker + i;
                if request_index >= total {
                    break;
                }
                let resume_at = *cooldown.lock().await;
                tokio::time::sleep_until(resume_at).await;
                let body = method_body(request_index);
                let start = Instant::now();
                match client.post(&url).json(&body).send().await {
                    Ok(resp) => {
                        let status = resp.status();
                        let body = resp.json::<Value>().await;
                        let ms = start.elapsed().as_secs_f64() * 1000.0;
                        if status.as_u16() == 429 {
                            local.record_throttle();
                            *cooldown.lock().await =
                                tokio::time::Instant::now() + Duration::from_secs(10);
                        } else if status.is_success()
                            && body.as_ref().is_ok_and(|body| {
                                body.get("result").is_some() && body.get("error").is_none()
                            })
                        {
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
        agg.merge(res?);
    }
    Ok(agg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Context;
    use axum::{routing::post, Json, Router};

    #[tokio::test]
    async fn exact_request_count_and_http_200_rpc_errors() -> anyhow::Result<()> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .context(concat!(file!(), ":", line!()))?;
        let endpoint = format!(
            "http://{}",
            listener
                .local_addr()
                .context(concat!(file!(), ":", line!()))?
        );
        let app = Router::new().route(
            "/",
            post(|| async {
                Json(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "error": {"code": -32601, "message": "unsupported"}
                }))
            }),
        );
        let server = tokio::spawn(async move { axum::serve(listener, app).await });
        let stats = tokio::time::timeout(Duration::from_secs(5), bench_level(&endpoint, 3, 5))
            .await
            .context(concat!(file!(), ":", line!()))??;
        assert_eq!(stats.total(), 5);
        assert_eq!(stats.errors, 5);
        assert_eq!(stats.ok(), 0);
        server.abort();
        Ok(())
    }
}
