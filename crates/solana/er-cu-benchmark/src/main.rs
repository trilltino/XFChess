//! ER CU Benchmark CLI runner

use clap::{Parser, ValueEnum};
use er_cu_benchmark::{
    base_client,
    cost_reporter::{export_json, generate_cost_report, print_cost_report},
    cu_logger::CuLogger,
    er_client,
    game_flows::{run_1v1_game_flow, run_swiss_tournament_flow},
    keygen::{
        fund_children, generate_child_keypairs, load_or_generate_master_keypair, reclaim_surplus,
    },
    moves::generate_100_move_sequence,
    PROGRAM_ID,
};
use solana_sdk::{pubkey::Pubkey, signature::Signer};
use std::time::Duration;
use tokio::time::sleep;

#[derive(Parser, Debug, Clone)]
#[command(name = "er-cu-benchmark")]
#[command(about = "XFChess ER Compute Unit Benchmark Suite")]
#[command(version = "1.0.0")]
struct Cli {
    #[arg(short, long, value_enum, default_value = "1v1")]
    mode: TestMode,

    #[arg(short, long, default_value = "16")]
    size: u16,

    #[arg(long)]
    all: bool,

    #[arg(long)]
    skip_funding: bool,

    #[arg(long)]
    skip_reclaim: bool,

    #[arg(long)]
    export: Option<String>,

    #[arg(long)]
    init: bool,
}

#[derive(ValueEnum, Clone, Debug)]
enum TestMode {
    OneVOne,
    Swiss,
    All,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.init {
        println!("╔══════════════════════════════════════════════════════════╗");
        println!("║     XFChess ER CU Benchmark - Keypair Setup            ║");
        println!("╚══════════════════════════════════════════════════════════╝");
        println!();
        let master = load_or_generate_master_keypair()?;
        println!();
        println!("   Master keypair ready.");
        println!();
        println!("   SEND DEVNET SOL TO THIS ADDRESS TO FUND TESTS:");
        println!("   {}", master.pubkey());
        println!();
        println!("   https://faucet.solana.com/");
        println!();
        println!("   When funded, run tests with:");
        println!("   cargo run --bin er-cu-benchmark -- --mode 1v1");
        return Ok(());
    }

    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║     XFChess ER Compute Unit Benchmark Suite              ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();

    let program_id: Pubkey = PROGRAM_ID.parse()?;
    println!("   Program ID: {}", program_id);

    let master = load_or_generate_master_keypair()?;
    let master_balance = base_client().get_balance(&master.pubkey())?;
    println!(
        "   Master balance: {} SOL",
        master_balance as f64 / 1_000_000_000.0
    );

    if master_balance < 50_000_000 {
        println!();
        println!("   Master balance too low. Fund this address and retry:");
        println!("   {}", master.pubkey());
        println!();
        println!("   Or generate a fresh keypair with:");
        println!("   cargo run --bin er-cu-benchmark -- --init");
        return Err(anyhow::anyhow!("Insufficient balance"));
    }

    match cli.mode {
        TestMode::OneVOne => run_1v1_test(&master, program_id, cli).await?,
        TestMode::Swiss => {
            if cli.all {
                run_all_swiss_sizes(&master, program_id, cli).await?;
            } else {
                run_swiss_test(&master, program_id, cli.size, cli).await?;
            }
        }
        TestMode::All => {
            run_1v1_test(&master, program_id, cli.clone()).await?;
            if cli.all {
                run_all_swiss_sizes(&master, program_id, cli).await?;
            } else {
                run_swiss_test(&master, program_id, cli.size, cli).await?;
            }
        }
    }

    println!("\n   All tests complete!");
    Ok(())
}

async fn run_1v1_test(
    master: &solana_sdk::signature::Keypair,
    program_id: Pubkey,
    cli: Cli,
) -> anyhow::Result<()> {
    println!("\n══════════════════════════════════════════════════════════");
    println!("  1V1 GAME TEST");
    println!("══════════════════════════════════════════════════════════");

    let base_rpc = base_client();
    let er_rpc = er_client();
    let children = generate_child_keypairs(2);
    let white = &children[0];
    let black = &children[1];

    if !cli.skip_funding {
        println!("\n   Funding player wallets...");
        fund_children(&base_rpc, master, &children).await?;
    }

    println!("\n   Generating 100-move checkmate sequence...");
    let _moves = generate_100_move_sequence();

    let mut logger = CuLogger::new();
    let total_cu = run_1v1_game_flow(
        &base_rpc,
        &er_rpc,
        program_id,
        master,
        white,
        black,
        &mut logger,
    )
    .await?;

    logger.print_summary();
    let report = generate_cost_report(&logger, "1v1_game");
    print_cost_report(&report);

    if let Some(path) = &cli.export {
        let json = export_json(&report);
        std::fs::write(format!("{}_1v1.json", path), json)?;
        println!("   Exported to {}_1v1.json", path);
    }

    if !cli.skip_reclaim {
        println!("\n   Reclaiming surplus...");
        reclaim_surplus(&base_rpc, master, &children).await?;
    }

    println!("\n   1v1 test complete. Total CU: {}", total_cu);
    Ok(())
}

async fn run_swiss_test(
    master: &solana_sdk::signature::Keypair,
    program_id: Pubkey,
    size: u16,
    cli: Cli,
) -> anyhow::Result<()> {
    println!("\n══════════════════════════════════════════════════════════");
    println!("  SWISS TOURNAMENT TEST ({} players)", size);
    println!("══════════════════════════════════════════════════════════");

    let base_rpc = base_client();
    let er_rpc = er_client();
    let valid_sizes = [8u16, 16, 32, 64, 128, 256];
    if !valid_sizes.contains(&size) {
        return Err(anyhow::anyhow!(
            "Invalid size: {}. Must be one of: {:?}",
            size,
            valid_sizes
        ));
    }

    let children = generate_child_keypairs(size as usize);
    if !cli.skip_funding {
        println!("\n   Funding {} player wallets...", size);
        fund_children(&base_rpc, master, &children).await?;
    }

    let mut logger = CuLogger::new();
    let total_cu = run_swiss_tournament_flow(
        &base_rpc,
        &er_rpc,
        program_id,
        master,
        &children,
        size,
        &mut logger,
    )
    .await?;

    logger.print_summary();
    let report = generate_cost_report(&logger, &format!("swiss_{}_players", size));
    print_cost_report(&report);

    if let Some(path) = &cli.export {
        let json = export_json(&report);
        std::fs::write(format!("{}_swiss_{}.json", path, size), json)?;
        println!("   Exported to {}_swiss_{}.json", path, size);
    }

    if !cli.skip_reclaim {
        println!("\n   Reclaiming surplus from {} players...", size);
        reclaim_surplus(&base_rpc, master, &children).await?;
    }

    println!(
        "\n   Swiss {}-player test complete. Total CU: {}",
        size, total_cu
    );
    Ok(())
}

async fn run_all_swiss_sizes(
    master: &solana_sdk::signature::Keypair,
    program_id: Pubkey,
    cli: Cli,
) -> anyhow::Result<()> {
    let sizes = [8u16, 16, 32, 64, 128];
    println!("\n══════════════════════════════════════════════════════════");
    println!("  RUNNING ALL SWISS TOURNAMENT SIZES");
    println!("══════════════════════════════════════════════════════════");

    for size in sizes {
        match run_swiss_test(master, program_id, size, cli.clone()).await {
            Ok(_) => println!("      Size {} complete\n", size),
            Err(e) => {
                eprintln!("      Size {} failed: {}", size, e);
                println!("      Continuing...\n");
            }
        }
        sleep(Duration::from_secs(2)).await;
    }

    println!("\n   All Swiss tournament sizes tested!");
    Ok(())
}
