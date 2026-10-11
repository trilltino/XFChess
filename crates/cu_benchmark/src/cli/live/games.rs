use cu_benchmark::chain::{
    base_client, er_client,
    flows::{run_1v1_game_flow, run_global_session_1v1_game_flow},
    wallets::{fund_children, generate_child_keypairs, reclaim_surplus},
};
use cu_benchmark::cost::measurements::MeasurementLog;
use solana_sdk::pubkey::Pubkey;

use super::{write_report, Cli};

pub(super) async fn run_1v1_test(
    master: &solana_sdk::signature::Keypair,
    program_id: Pubkey,
    cli: Cli,
) -> anyhow::Result<()> {
    println!("  1V1 GAME TEST");

    let base_rpc = base_client();
    let er_rpc = er_client();
    let children = generate_child_keypairs(2);
    let white = &children[0];
    let black = &children[1];

    if !cli.skip_funding {
        println!("\n   Funding player wallets...");
        fund_children(&base_rpc, master, &children).await?;
    }

    let mut logger = MeasurementLog::new();
    run_1v1_game_flow(
        &base_rpc,
        &er_rpc,
        program_id,
        master,
        white,
        black,
        &mut logger,
    )
    .await?;

    write_report(
        &mut logger,
        &base_rpc,
        "1v1_game",
        cli.export.as_ref().map(|path| format!("{}_1v1.json", path)),
    )?;

    if !cli.skip_reclaim {
        println!("\n   Reclaiming surplus...");
        reclaim_surplus(&base_rpc, master, &children).await?;
    }

    println!("\n   1v1 test complete. Total CU: {}", logger.total_cu());
    Ok(())
}

pub(super) async fn run_global_session_test(
    master: &solana_sdk::signature::Keypair,
    program_id: Pubkey,
    cli: Cli,
) -> anyhow::Result<()> {
    println!("  GLOBAL-SESSION 1V1 GAME TEST");

    let base_rpc = base_client();
    let er_rpc = er_client();
    let children = generate_child_keypairs(2);
    let white = &children[0];
    let black = &children[1];

    if !cli.skip_funding {
        println!("\n   Funding player wallets...");
        fund_children(&base_rpc, master, &children).await?;
    }

    let mut logger = MeasurementLog::new();
    run_global_session_1v1_game_flow(
        &base_rpc,
        &er_rpc,
        program_id,
        master,
        white,
        black,
        &mut logger,
    )
    .await?;

    write_report(
        &mut logger,
        &base_rpc,
        "global_session_1v1_game",
        cli.export
            .as_ref()
            .map(|path| format!("{}_global_session.json", path)),
    )?;

    if !cli.skip_reclaim {
        println!("\n   Reclaiming surplus...");
        reclaim_surplus(&base_rpc, master, &children).await?;
    }

    println!(
        "\n   Global-session 1v1 test complete. Total CU: {}",
        logger.total_cu()
    );
    Ok(())
}
