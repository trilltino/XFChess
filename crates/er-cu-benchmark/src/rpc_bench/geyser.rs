use std::collections::HashMap;
use std::time::{Duration, Instant};

use anyhow::Context;
use futures::StreamExt;
use yellowstone_grpc_client::GeyserGrpcClient;
use yellowstone_grpc_proto::geyser::{
    subscribe_update::UpdateOneof, SubscribeRequest, SubscribeRequestFilterSlots,
};

use super::redact_url;

pub async fn run(
    endpoint: &str,
    x_token: Option<String>,
    max_messages: usize,
    window_secs: u64,
) -> anyhow::Result<()> {
    println!("\n╔══════════════════════════════════════════════════════════╗");
    println!("║  GEYSER gRPC CONNECTIVITY PROBE                              ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!("   endpoint: {}", redact_url(endpoint));

    let connect_start = Instant::now();
    let mut builder = GeyserGrpcClient::build_from_shared(endpoint.to_string())
        .context("invalid gRPC endpoint")?;
    if let Some(token) = x_token {
        builder = builder.x_token(Some(token)).context("set x-token")?;
    }
    let mut client = builder
        .connect()
        .await
        .context("gRPC connect failed (check endpoint/token/tier)")?;
    println!(
        "   connected in {:.0} ms",
        connect_start.elapsed().as_secs_f64() * 1000.0
    );

    let mut slots = HashMap::new();
    slots.insert("bench".to_string(), SubscribeRequestFilterSlots::default());
    let request = SubscribeRequest {
        slots,
        ..Default::default()
    };

    let (_tx, mut stream) = client
        .subscribe_with_request(Some(request))
        .await
        .context("subscribe failed")?;

    let sub_start = Instant::now();
    let mut first_msg: Option<Duration> = None;
    let mut slot_updates = 0usize;
    let mut total_msgs = 0usize;

    println!("   subscribed — waiting for slot pushes (<= {window_secs}s)…");

    let deadline = Instant::now() + Duration::from_secs(window_secs);
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() || total_msgs >= max_messages {
            break;
        }
        match tokio::time::timeout(remaining, stream.next()).await {
            Ok(Some(Ok(update))) => {
                total_msgs += 1;
                if first_msg.is_none() {
                    first_msg = Some(sub_start.elapsed());
                }
                if let Some(UpdateOneof::Slot(slot)) = update.update_oneof {
                    slot_updates += 1;
                    if slot_updates <= 3 {
                        println!("   slot push: {}", slot.slot);
                    }
                }
            }
            Ok(Some(Err(e))) => {
                anyhow::bail!("stream error: {e}");
            }
            Ok(None) => break, // stream closed
            Err(_) => break,   // window elapsed
        }
    }

    println!("\n── GEYSER SUMMARY ──");
    match first_msg {
        Some(d) => println!(
            "   time-to-first-message: {:.0} ms",
            d.as_secs_f64() * 1000.0
        ),
        None => println!("   time-to-first-message: (none received)"),
    }
    println!("   total messages: {total_msgs}  (slot updates: {slot_updates})");
    let secs = sub_start.elapsed().as_secs_f64().max(1e-6);
    println!("   stream rate: {:.1} msg/s", total_msgs as f64 / secs);

    if total_msgs == 0 {
        anyhow::bail!("no messages received — Geyser may not be enabled on this tier");
    }
    println!("\n   Push streaming works → settlement_worker can go poll → subscribe.");
    Ok(())
}
