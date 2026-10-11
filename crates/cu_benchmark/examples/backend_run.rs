use anyhow::{ensure, Context, Result};
use clap::Parser;
use cu_benchmark::system::{report, runner, scenario::Scenario};
use serde_json::{json, Value};
use solana_sdk::signature::{Keypair, Signer};
use std::{
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Parser)]
struct Args {
    #[arg(long)]
    scenario: PathBuf,
    #[arg(long)]
    export: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let mut scenario: Scenario = serde_json::from_slice(&std::fs::read(&args.scenario)?)?;
    scenario.validate()?;
    ensure!(
        !scenario.sends_transactions && scenario.chain.is_none(),
        "this example supports casual HTTP games only"
    );
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(scenario.timeout_ms))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    for side in ["white", "black"] {
        let key = Keypair::new();
        let wallet = key.pubkey().to_string();
        let message = format!("xfchess:register:{timestamp}");
        let response = client.post(format!("{}/api/auth/register", scenario.backend_url.trim_end_matches('/')))
            .json(&json!({"wallet":wallet,"signature":key.sign_message(message.as_bytes()).to_string(),
                "timestamp":timestamp,"username":format!("bench_{}", &wallet[..12])}))
            .send().await.context("registration request failed")?;
        ensure!(
            response.status().is_success(),
            "test actor registration returned {}",
            response.status()
        );
        let body: Value = response
            .json()
            .await
            .context("invalid registration response")?;
        let token = body
            .get("token")
            .and_then(Value::as_str)
            .context("registration token missing")?;
        std::env::set_var(format!("BENCH_{}_WALLET", side.to_uppercase()), &wallet);
        std::env::set_var(format!("BENCH_{}_TOKEN", side.to_uppercase()), token);
        scenario.variables.insert(
            format!("{side}_wallet"),
            json!(format!("${{env:BENCH_{}_WALLET}}", side.to_uppercase())),
        );
        scenario.variables.insert(
            format!("{side}_token"),
            json!(format!("${{env:BENCH_{}_TOKEN}}", side.to_uppercase())),
        );
    }
    let observed = runner::execute(&scenario, false).await?;
    let report = report::build(&scenario, observed)?;
    std::fs::write(&args.export, serde_json::to_vec_pretty(&report)?)?;
    for stage in &report.stages {
        println!(
            "{}: {}/{} games, peak {}, p95 {:.1} ms, {:?}",
            stage.observed.name,
            stage.observed.completed_games,
            stage.observed.requested_games,
            stage.observed.peak_concurrency,
            stage.request_p95_ms.unwrap_or_default(),
            stage.capacity
        );
    }
    println!("Report: {}", args.export.display());
    Ok(())
}
