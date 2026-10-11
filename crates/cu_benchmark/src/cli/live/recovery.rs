use cu_benchmark::chain::{
    base_client, er_client,
    recovery::{run_crank_liveness_drill, run_stuck_delegation_drill},
    wallets::{
        fund_children, generate_child_keypairs, load_dispute_authority_keypair, reclaim_surplus,
    },
};
use solana_sdk::pubkey::Pubkey;

use super::Cli;

pub(super) async fn run_crank_drill(
    master: &solana_sdk::signature::Keypair,
    program_id: Pubkey,
    cli: Cli,
) -> anyhow::Result<()> {
    println!("  CRANK LIVENESS DRILL (real devnet, ~1-2 min)");

    let base_rpc = base_client();
    let er_rpc = er_client();
    let children = generate_child_keypairs(2);
    let white = &children[0];
    let black = &children[1];

    if !cli.skip_funding {
        println!("\n   Funding player wallets...");
        fund_children(&base_rpc, master, &children).await?;
    }

    run_crank_liveness_drill(&base_rpc, &er_rpc, program_id, white, black).await?;

    if !cli.skip_reclaim {
        println!("\n   Reclaiming surplus...");
        reclaim_surplus(&base_rpc, master, &children).await?;
    }

    Ok(())
}

pub(super) async fn run_recovery_drill(
    master: &solana_sdk::signature::Keypair,
    program_id: Pubkey,
    cli: Cli,
) -> anyhow::Result<()> {
    println!("  ER-UNAVAILABILITY RECOVERY DRILL (real devnet, ~60-70 min)");

    let dispute_authority = load_dispute_authority_keypair()?;
    let base_rpc = base_client();
    let children = generate_child_keypairs(2);
    let white = &children[0];
    let black = &children[1];

    if !cli.skip_funding {
        println!("\n   Funding player wallets...");
        fund_children(&base_rpc, master, &children).await?;
    }

    run_stuck_delegation_drill(&base_rpc, program_id, white, black, &dispute_authority).await?;

    // Leave recovered payouts in place for inspection; do not reclaim surplus.
    Ok(())
}
