mod games;
mod recovery;
mod tournaments;

use cu_benchmark::chain::{
    base_client, er_client, wallets::load_or_generate_master_keypair, PROGRAM_ID,
};
use cu_benchmark::cost::{
    measurements::MeasurementLog,
    report::{export_json, generate_cost_report, print_cost_report},
};
use solana_sdk::{pubkey::Pubkey, signature::Signer};

use super::args::{Cli, TestMode};
use games::{run_1v1_test, run_global_session_test};
use recovery::{run_crank_drill, run_recovery_drill};
use tournaments::{
    run_all_single_elim_sizes, run_all_swiss_sizes, run_single_elim_test, run_swiss_test,
};

pub(super) async fn run(cli: Cli) -> anyhow::Result<()> {
    anyhow::ensure!(
        cli.init || cli.live,
        "use --scenario <scenario.json>, --estimate <workload.json>, --init, or --live --mode <mode>"
    );

    if cli.init {
        println!("cu_benchmark: keypair setup");
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
        println!("   cargo run -p cu_benchmark -- --live --mode 1v1");
        return Ok(());
    }
    println!("cu_benchmark: live measurements");
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
        println!("   cargo run --bin cu_benchmark -- --init");
        return Err(anyhow::anyhow!("Insufficient balance"));
    }

    match cli.mode {
        TestMode::OneVOne => run_1v1_test(&master, program_id, cli).await?,
        TestMode::GlobalSession => run_global_session_test(&master, program_id, cli).await?,
        TestMode::Swiss => {
            if cli.all {
                run_all_swiss_sizes(&master, program_id, cli).await?;
            } else {
                run_swiss_test(&master, program_id, cli.size, cli).await?;
            }
        }
        TestMode::SingleElim => {
            if cli.all {
                run_all_single_elim_sizes(&master, program_id, cli).await?;
            } else {
                run_single_elim_test(&master, program_id, cli.size, cli).await?;
            }
        }
        TestMode::All => {
            run_1v1_test(&master, program_id, cli.clone()).await?;
            if cli.all {
                run_all_swiss_sizes(&master, program_id, cli.clone()).await?;
                run_all_single_elim_sizes(&master, program_id, cli).await?;
            } else {
                run_swiss_test(&master, program_id, cli.size, cli.clone()).await?;
                run_single_elim_test(&master, program_id, cli.size, cli).await?;
            }
        }
        TestMode::CrankDrill => run_crank_drill(&master, program_id, cli).await?,
        TestMode::RecoveryDrill => run_recovery_drill(&master, program_id, cli).await?,
    }

    println!("\n   All tests complete!");
    Ok(())
}

fn write_report(
    logger: &mut MeasurementLog,
    rpc: &solana_client::rpc_client::RpcClient,
    scenario: &str,
    export: Option<String>,
) -> anyhow::Result<()> {
    let warnings = logger.measure(rpc, &er_client());
    logger.print_summary();
    let mut report = generate_cost_report(logger, scenario);
    report.warnings.extend(warnings);
    print_cost_report(&report);
    if let Some(path) = export {
        std::fs::write(&path, export_json(&report)?)?;
        println!("Exported to {path}");
    }
    Ok(())
}
