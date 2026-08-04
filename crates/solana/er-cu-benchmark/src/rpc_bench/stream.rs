//! WebSocket pubsub streaming probe (Windows-friendly alternative to the gRPC
//! Geyser probe — no protobuf toolchain required).
//!
//! Subscribes to slot notifications over the endpoint's WebSocket pubsub and
//! reports time-to-first-message and stream rate. Like the Geyser probe, this
//! validates that `settlement_worker` could move from 30s polling to push-on-event
//! — just over standard pubsub instead of Yellowstone gRPC.

use std::time::{Duration, Instant};

use solana_client::nonblocking::pubsub_client::PubsubClient;
use tokio_stream::StreamExt;

use super::redact_url;

/// Convert an http(s) RPC URL to its ws(s) pubsub form (same host + token path).
pub fn to_ws(url: &str) -> String {
    if let Some(rest) = url.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = url.strip_prefix("http://") {
        format!("ws://{rest}")
    } else {
        url.to_string()
    }
}

/// Connect to the pubsub WS, subscribe to slots, observe up to `window` seconds
/// (stopping early after `max_messages`).
pub async fn run(ws_url: &str, max_messages: usize, window_secs: u64) -> anyhow::Result<()> {
    println!("\n╔══════════════════════════════════════════════════════════╗");
    println!("║  WEBSOCKET PUBSUB STREAM PROBE                              ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!("   endpoint: {}", redact_url(ws_url));

    let connect_start = Instant::now();
    let client = PubsubClient::new(ws_url)
        .await
        .map_err(|e| anyhow::anyhow!("ws connect failed: {e}"))?;
    println!(
        "   connected in {:.0} ms",
        connect_start.elapsed().as_secs_f64() * 1000.0
    );

    let (mut stream, unsubscribe) = client
        .slot_subscribe()
        .await
        .map_err(|e| anyhow::anyhow!("slot_subscribe failed: {e}"))?;

    let sub_start = Instant::now();
    let mut first_msg: Option<Duration> = None;
    let mut count = 0usize;
    println!("   subscribed — waiting for slot pushes (<= {window_secs}s)…");

    let deadline = Instant::now() + Duration::from_secs(window_secs);
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() || count >= max_messages {
            break;
        }
        match tokio::time::timeout(remaining, stream.next()).await {
            Ok(Some(slot_info)) => {
                count += 1;
                if first_msg.is_none() {
                    first_msg = Some(sub_start.elapsed());
                }
                if count <= 3 {
                    println!("   slot push: {}", slot_info.slot);
                }
            }
            Ok(None) => break, // stream closed
            Err(_) => break,   // window elapsed
        }
    }

    unsubscribe().await;

    println!("\n── STREAM SUMMARY ──");
    match first_msg {
        Some(d) => println!(
            "   time-to-first-message: {:.0} ms",
            d.as_secs_f64() * 1000.0
        ),
        None => println!("   time-to-first-message: (none received)"),
    }
    let secs = sub_start.elapsed().as_secs_f64().max(1e-6);
    println!(
        "   messages: {count}  ·  rate: {:.1} msg/s",
        count as f64 / secs
    );

    if count == 0 {
        anyhow::bail!("no slot notifications — check the WS endpoint/token");
    }
    println!("\n   Push streaming works → settlement_worker can go poll → subscribe.");
    Ok(())
}
