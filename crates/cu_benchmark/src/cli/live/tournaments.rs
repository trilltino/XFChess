use cu_benchmark::chain::{
    base_client, er_client,
    flows::{run_single_elimination_tournament_flow, run_swiss_tournament_flow},
    wallets::{
        fund_children, generate_child_keypairs, load_tournament_authority_keypair, reclaim_surplus,
    },
};
use cu_benchmark::cost::measurements::MeasurementLog;
use solana_sdk::{pubkey::Pubkey, signature::Signer};
use std::time::Duration;
use tokio::time::sleep;

use super::{write_report, Cli};

pub(super) async fn run_swiss_test(
    master: &solana_sdk::signature::Keypair,
    program_id: Pubkey,
    size: u16,
    cli: Cli,
) -> anyhow::Result<()> {
    println!("  SWISS TOURNAMENT TEST ({} players)", size);

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

    let authority = funded_authority(&base_rpc)?;

    let mut logger = MeasurementLog::new();
    run_swiss_tournament_flow(
        &base_rpc,
        &er_rpc,
        program_id,
        &authority,
        &children,
        size,
        &mut logger,
    )
    .await?;

    write_report(
        &mut logger,
        &base_rpc,
        &format!("swiss_{}_players", size),
        cli.export
            .as_ref()
            .map(|path| format!("{}_swiss_{}.json", path, size)),
    )?;

    if !cli.skip_reclaim {
        println!("\n   Reclaiming surplus from {} players...", size);
        reclaim_surplus(&base_rpc, master, &children).await?;
    }

    println!(
        "\n   Swiss {}-player test complete. Total CU: {}",
        size,
        logger.total_cu()
    );
    Ok(())
}

pub(super) async fn run_single_elim_test(
    master: &solana_sdk::signature::Keypair,
    program_id: Pubkey,
    size: u16,
    cli: Cli,
) -> anyhow::Result<()> {
    println!("  SINGLE-ELIMINATION TOURNAMENT TEST ({} players)", size);

    let base_rpc = base_client();
    // Bound single-elimination size because match completion requires sequential
    // transactions rather than parallel player-signed rounds.
    let valid_sizes = [2u16, 4, 8, 16];
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

    let authority = funded_authority(&base_rpc)?;

    let mut logger = MeasurementLog::new();
    run_single_elimination_tournament_flow(
        &base_rpc,
        program_id,
        &authority,
        &children,
        size,
        &mut logger,
    )
    .await?;

    write_report(
        &mut logger,
        &base_rpc,
        &format!("single_elim_{}_players", size),
        cli.export
            .as_ref()
            .map(|path| format!("{}_single_elim_{}.json", path, size)),
    )?;

    if !cli.skip_reclaim {
        println!("\n   Reclaiming surplus from {} players...", size);
        reclaim_surplus(&base_rpc, master, &children).await?;
    }

    println!(
        "\n   Single-elimination {}-player test complete. Total CU: {}",
        size,
        logger.total_cu()
    );
    Ok(())
}

pub(super) async fn run_all_single_elim_sizes(
    master: &solana_sdk::signature::Keypair,
    program_id: Pubkey,
    cli: Cli,
) -> anyhow::Result<()> {
    let sizes = [2u16, 4, 8, 16];
    println!("  RUNNING ALL SINGLE-ELIMINATION TOURNAMENT SIZES");

    for size in sizes {
        match run_single_elim_test(master, program_id, size, cli.clone()).await {
            Ok(_) => println!("      Size {} complete\n", size),
            Err(e) => {
                eprintln!("      Size {} failed: {}", size, e);
                println!("      Continuing...\n");
            }
        }
        sleep(Duration::from_secs(2)).await;
    }

    println!("\n   All single-elimination tournament sizes tested!");
    Ok(())
}

pub(super) async fn run_all_swiss_sizes(
    master: &solana_sdk::signature::Keypair,
    program_id: Pubkey,
    cli: Cli,
) -> anyhow::Result<()> {
    let sizes = [8u16, 16, 32, 64, 128];
    println!("  RUNNING ALL SWISS TOURNAMENT SIZES");

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

fn funded_authority(
    base_rpc: &solana_client::rpc_client::RpcClient,
) -> anyhow::Result<solana_sdk::signature::Keypair> {
    // Tournament setup requires the configured program authority, not the funded master wallet.
    let authority = load_tournament_authority_keypair()?;
    let authority_balance = base_rpc.get_balance(&authority.pubkey())?;
    println!(
        "   Tournament authority balance: {:.4} SOL",
        authority_balance as f64 / 1_000_000_000.0
    );
    if authority_balance < 200_000_000 {
        return Err(anyhow::anyhow!(
            "Tournament authority ({}) balance too low ({:.4} SOL) — fund it with devnet SOL \
             (it pays tournament rent + prize funding, not `master`)",
            authority.pubkey(),
            authority_balance as f64 / 1_000_000_000.0
        ));
    }

    Ok(authority)
}
