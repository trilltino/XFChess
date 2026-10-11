use super::args::Cli;
use cu_benchmark::chain::base_client;

pub(super) fn run(cli: &Cli, path: &std::path::Path) -> anyhow::Result<()> {
    use cu_benchmark::cost::{
        estimate::{estimate, Workload},
        quotes::{priority_quote, quote_rent},
    };
    let mut workload: Workload = serde_json::from_slice(&std::fs::read(path)?)?;
    anyhow::ensure!(
        !workload.variants.is_empty(),
        "workload needs at least one variant"
    );
    let rpc = base_client();
    let priority = if let Some(percentile) = cli.priority_percentile {
        let quote = priority_quote(&rpc, percentile, &cli.writable_account)?;
        for variant in &mut workload.variants {
            for batch in &mut variant.transactions {
                batch.cu_price_micro_lamports = quote.micro_lamports_per_cu;
            }
        }
        Some(quote)
    } else {
        None
    };
    if cli.quote_rent {
        quote_rent(&rpc, &mut workload)?;
    }
    let estimates = workload
        .variants
        .iter()
        .map(|variant| estimate(variant, workload.fee_profile))
        .collect::<anyhow::Result<Vec<_>>>()?;
    let report = serde_json::json!({
        "schema_version": 1,
        "kind": "estimate",
        "assumptions": workload,
        "rent_source": if cli.quote_rent { "rpc_quote" } else { "workload" },
        "priority_quote": priority,
        "sources": {
            "checked_at": "2026-10-10",
            "solana": "https://solana.com/docs/core/fees",
            "magicblock_current": "https://github.com/magicblock-labs/delegation-program/blob/3a6fbe210530ab912257aa485626f387e294748a/dlp-api/src/consts.rs",
            "magicblock_legacy": "https://github.com/magicblock-labs/delegation-program/blob/6898ef4b82ba1f2b6fbb5d91eca578729edbbeb8/dlp-api/src/consts.rs",
            "magicblock_validator": "https://github.com/magicblock-labs/magicblock-validator/tree/b38979db27715fdff7346b4bcf0829d5158f96d9",
            "triton": "https://docs.triton.one/chains/solana/improved-priority-fees-api"
        },
        "estimates": estimates,
        "scope": "Modeled app fees and storage funding. Wagers, prizes, provider subscriptions and validator operating costs excluded. Funding sums deposits before refunds; it is not peak concurrent capital."
    });
    let json = serde_json::to_string_pretty(&report)?;
    if let Some(path) = &cli.export {
        std::fs::write(path, &json)?;
    }
    println!("{json}");
    Ok(())
}
