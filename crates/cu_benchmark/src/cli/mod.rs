mod args;
mod estimate;
mod live;

use clap::Parser;

pub async fn run() -> anyhow::Result<()> {
    let cli = args::Cli::parse();
    if let Some(path) = &cli.scenario {
        return cu_benchmark::system::run(
            path,
            cli.execute,
            cli.allow_transactions,
            cli.observations.as_deref(),
            cli.export.as_deref(),
        )
        .await;
    }
    if let Some(path) = &cli.estimate {
        return estimate::run(&cli, path);
    }
    live::run(cli).await
}
