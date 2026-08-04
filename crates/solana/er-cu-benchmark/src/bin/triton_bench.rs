//! triton-bench — head-to-head RPC suite proving the Triton integration's value.
//!
//! Subcommands:
//!   read-load   read-RPC latency + 429 rate, Triton vs public devnet (ramping)
//!   tx-land     transaction submit/confirm timing, Triton vs public devnet
//!   geyser      Yellowstone gRPC push-streaming probe (needs --features geyser)
//!   all         read-load + tx-land (+ geyser if compiled in)
//!
//! Examples (PowerShell):
//!   $env:SOLANA_RPC_URL="https://<host>.devnet.rpcpool.com/<token>"
//!   cargo run -p er-cu-benchmark --bin triton-bench -- read-load
//!   cargo run -p er-cu-benchmark --bin triton-bench -- tx-land --count 10
//!   cargo run -p er-cu-benchmark --bin triton-bench --features geyser -- geyser

use clap::{Parser, Subcommand};
use er_cu_benchmark::{
    keygen::load_or_generate_master_keypair,
    rpc_bench::{read_load, stream, tx_land},
};
use solana_client::rpc_client::RpcClient;
use solana_sdk::signature::Signer;

const PUBLIC_DEVNET: &str = "https://api.devnet.solana.com";

#[derive(Parser, Debug)]
#[command(name = "triton-bench")]
#[command(about = "Triton vs public-devnet RPC benchmark suite")]
struct Cli {
    /// Triton RPC URL (defaults to $SOLANA_RPC_URL).
    #[arg(long, env = "SOLANA_RPC_URL")]
    triton_url: Option<String>,

    /// Baseline RPC URL to compare against.
    #[arg(long, default_value = PUBLIC_DEVNET)]
    baseline_url: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Read-RPC latency + 429 rate under ramping concurrency.
    ReadLoad {
        /// Requests fired per concurrency level.
        #[arg(long, default_value = "200")]
        requests: usize,
        /// Comma-separated concurrency levels.
        #[arg(long, default_value = "1,8,16,32,64,128")]
        levels: String,
    },
    /// Transaction submit + confirm timing (needs a funded master keypair).
    TxLand {
        /// Number of memo transactions per endpoint.
        #[arg(long, default_value = "10")]
        count: usize,
    },
    /// WebSocket pubsub stream probe (Windows-friendly; no protobuf toolchain).
    Stream {
        /// WS URL (defaults to the Triton URL converted to wss://).
        #[arg(long)]
        ws_url: Option<String>,
        /// Stop after this many messages.
        #[arg(long, default_value = "20")]
        messages: usize,
        /// Max seconds to observe the stream.
        #[arg(long, default_value = "15")]
        window: u64,
    },
    /// Geyser gRPC push-streaming connectivity probe.
    Geyser {
        /// gRPC endpoint (defaults to $GEYSER_GRPC_URL, else the Triton host on :443).
        #[arg(long, env = "GEYSER_GRPC_URL")]
        grpc_url: Option<String>,
        /// x-token for gRPC auth (defaults to $GEYSER_X_TOKEN, else the Triton URL token).
        #[arg(long, env = "GEYSER_X_TOKEN")]
        x_token: Option<String>,
        /// Stop after this many messages.
        #[arg(long, default_value = "20")]
        messages: usize,
        /// Max seconds to observe the stream.
        #[arg(long, default_value = "15")]
        window: u64,
    },
    /// Run read-load + tx-land (+ geyser if compiled in).
    All,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let triton = cli
        .triton_url
        .clone()
        .ok_or_else(|| anyhow::anyhow!("set --triton-url or $SOLANA_RPC_URL"))?;

    match &cli.command {
        Command::ReadLoad { requests, levels } => {
            run_read_load(&triton, &cli.baseline_url, *requests, levels).await;
        }
        Command::TxLand { count } => {
            run_tx_land(&triton, &cli.baseline_url, *count).await?;
        }
        Command::Stream {
            ws_url,
            messages,
            window,
        } => {
            let url = ws_url.clone().unwrap_or_else(|| stream::to_ws(&triton));
            stream::run(&url, *messages, *window).await?;
        }
        Command::Geyser {
            grpc_url,
            x_token,
            messages,
            window,
        } => {
            run_geyser(
                &triton,
                grpc_url.clone(),
                x_token.clone(),
                *messages,
                *window,
            )
            .await?;
        }
        Command::All => {
            run_read_load(&triton, &cli.baseline_url, 200, "1,8,16,32,64,128").await;
            run_tx_land(&triton, &cli.baseline_url, 10).await?;
            // WS stream probe builds everywhere; gRPC Geyser only with --features geyser.
            if let Err(e) = stream::run(&stream::to_ws(&triton), 20, 15).await {
                eprintln!("   stream probe: {e}");
            }
            run_geyser(&triton, None, None, 20, 15).await?;
        }
    }

    Ok(())
}

fn parse_levels(s: &str) -> Vec<usize> {
    s.split(',')
        .filter_map(|p| p.trim().parse::<usize>().ok())
        .filter(|&n| n > 0)
        .collect()
}

async fn run_read_load(triton: &str, baseline: &str, requests: usize, levels: &str) {
    let levels = parse_levels(levels);
    let targets = vec![
        read_load::Target {
            name: "Triton".into(),
            url: triton.to_string(),
        },
        read_load::Target {
            name: "public devnet".into(),
            url: baseline.to_string(),
        },
    ];
    read_load::run(&targets, &levels, requests).await;
}

async fn run_tx_land(triton: &str, baseline: &str, count: usize) -> anyhow::Result<()> {
    let master = load_or_generate_master_keypair()?;

    // Cheap balance gate: memo txs cost the base fee (5000 lamports) each.
    let rpc = RpcClient::new(triton.to_string());
    let balance = rpc.get_balance(&master.pubkey()).unwrap_or(0);
    let needed = (count as u64 * 2) * 5_000 + 1_000_000;
    if balance < needed {
        anyhow::bail!(
            "master {} has {:.4} SOL — fund it (https://faucet.solana.com/) then retry",
            master.pubkey(),
            balance as f64 / 1_000_000_000.0
        );
    }
    println!(
        "   master {} · {:.4} SOL",
        master.pubkey(),
        balance as f64 / 1_000_000_000.0
    );

    println!("\n╔══════════════════════════════════════════════════════════╗");
    println!("║  TX LANDING TEST  (submit + confirm timing)                  ║");
    println!("╚══════════════════════════════════════════════════════════╝");

    let triton = triton.to_string();
    let baseline = baseline.to_string();
    // RpcClient is blocking — run the whole comparison off the async runtime.
    let reports = tokio::task::spawn_blocking(move || {
        vec![
            tx_land::run("Triton", &triton, &master, count),
            tx_land::run("public devnet", &baseline, &master, count),
        ]
    })
    .await?;

    tx_land::print_summary(&reports);
    Ok(())
}

/// Derive a sensible gRPC URL + token from the Triton HTTP URL when not given.
#[cfg(feature = "geyser")]
fn derive_geyser(
    triton: &str,
    grpc_url: Option<String>,
    x_token: Option<String>,
) -> (String, Option<String>) {
    // Token is the last path segment of the Triton URL (…rpcpool.com/<token>).
    let token = x_token.or_else(|| {
        triton
            .rsplit('/')
            .next()
            .filter(|s| s.len() >= 16)
            .map(|s| s.to_string())
    });
    // gRPC host = the HTTP host without the token path, on https (tonic uses :443).
    let url = grpc_url.unwrap_or_else(|| {
        let host = triton
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .split('/')
            .next()
            .unwrap_or(triton);
        format!("https://{host}")
    });
    (url, token)
}

#[cfg(feature = "geyser")]
async fn run_geyser(
    triton: &str,
    grpc_url: Option<String>,
    x_token: Option<String>,
    messages: usize,
    window: u64,
) -> anyhow::Result<()> {
    use er_cu_benchmark::rpc_bench::geyser;
    let (url, token) = derive_geyser(triton, grpc_url, x_token);
    geyser::run(&url, token, messages, window).await
}

#[cfg(not(feature = "geyser"))]
async fn run_geyser(
    _triton: &str,
    _grpc_url: Option<String>,
    _x_token: Option<String>,
    _messages: usize,
    _window: u64,
) -> anyhow::Result<()> {
    println!("\n   Geyser probe not compiled in.");
    println!("   Rebuild with: cargo run -p er-cu-benchmark --bin triton-bench --features geyser -- geyser");
    Ok(())
}
