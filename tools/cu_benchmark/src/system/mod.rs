pub mod collectors;
pub mod pricing;
pub mod report;
pub mod runner;
pub mod scenario;

use anyhow::{Context, Result};
use std::path::Path;

pub async fn run(
    path: &Path,
    execute: bool,
    allow_transactions: bool,
    observations: Option<&Path>,
    export: Option<&str>,
) -> Result<()> {
    let scenario: scenario::Scenario =
        serde_json::from_slice(&std::fs::read(path)?).context("invalid scenario")?;
    scenario.validate()?;
    let stages = if execute {
        runner::execute(&scenario, allow_transactions).await?
    } else if let Some(path) = observations {
        let previous: report::SystemReport = serde_json::from_slice(&std::fs::read(path)?)?;
        anyhow::ensure!(previous.schema_version == 1, "unsupported report version");
        anyhow::ensure!(
            previous.kind == "measurement" && !previous.stages.is_empty(),
            "a measured report is required"
        );
        anyhow::ensure!(
            previous.scenario == scenario.name,
            "scenario name differs from observations"
        );
        anyhow::ensure!(previous.workload_fingerprint == report::fingerprint(&scenario)?, "workload differs from observations; pricing and limits may change, execution inputs must match");
        previous
            .stages
            .into_iter()
            .map(|stage| stage.observed)
            .collect()
    } else {
        Vec::new()
    };
    let report = report::build(&scenario, stages)?;
    let json = serde_json::to_string_pretty(&report)?;
    if let Some(path) = export {
        std::fs::write(path, &json)?;
    }
    println!("{json}");
    Ok(())
}
