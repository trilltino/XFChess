//! Master keypair generation and child wallet funding.

use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    native_token::LAMPORTS_PER_SOL,
    signature::{Keypair, Signer},
    system_instruction,
    transaction::Transaction,
};
use std::fs;
use std::path::Path;

use crate::{CHILD_FUNDING_AMOUNT, CHILDREN_KEYPAIR_PATH, MASTER_KEYPAIR_PATH, MASTER_MIN_BALANCE};

/// Load the master keypair from disk or generate a new one.
pub fn load_or_generate_master_keypair() -> anyhow::Result<Keypair> {
    let path = Path::new(MASTER_KEYPAIR_PATH);

    if path.exists() {
        let data = fs::read_to_string(path)?;
        let bytes: Vec<u8> = serde_json::from_str(&data)?;
        let keypair = Keypair::from_bytes(&bytes)?;
        println!("   Loaded existing master keypair: {}", keypair.pubkey());
        return Ok(keypair);
    }

    let keypair = Keypair::new();
    let bytes = keypair.to_bytes().to_vec();
    let json = serde_json::to_string(&bytes)?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(path, json)?;
    println!("   Generated new master keypair: {}", keypair.pubkey());
    println!("   Saved to: {}", path.display());

    Ok(keypair)
}

/// Generate N child keypairs for test participants.
/// Loads from disk if previously saved, otherwise generates fresh and persists.
pub fn generate_child_keypairs(count: usize) -> Vec<Keypair> {
    let path = Path::new(CHILDREN_KEYPAIR_PATH);

    if path.exists() {
        if let Ok(data) = fs::read_to_string(path) {
            if let Ok(arr) = serde_json::from_str::<Vec<Vec<u8>>>(&data) {
                let keypairs: Vec<Keypair> = arr.iter()
                    .filter_map(|b| Keypair::from_bytes(b).ok())
                    .collect();
                if keypairs.len() == count {
                    println!("   Loaded {} existing child keypairs from {}", count, path.display());
                    return keypairs;
                }
                println!("   Found {} saved child keypairs but need {}. Regenerating...", keypairs.len(), count);
            }
        }
    }

    let keypairs: Vec<Keypair> = (0..count).map(|_| Keypair::new()).collect();

    let bytes_arr: Vec<Vec<u8>> = keypairs.iter().map(|k| k.to_bytes().to_vec()).collect();
    if let Ok(json) = serde_json::to_string(&bytes_arr) {
        let _ = fs::write(path, json);
        println!("   Generated and saved {} child keypairs to {}", count, path.display());
    }

    keypairs
}

/// Fund all child wallets from the master keypair.
pub async fn fund_children(
    rpc: &RpcClient,
    master: &Keypair,
    children: &[Keypair],
) -> anyhow::Result<()> {
    let master_balance = rpc.get_balance(&master.pubkey())?;
    let total_needed = CHILD_FUNDING_AMOUNT * children.len() as u64;

    if master_balance < total_needed + MASTER_MIN_BALANCE {
        return Err(anyhow::anyhow!(
            "Master balance ({:.4} SOL) insufficient to fund {} children ({:.4} SOL needed)",
            master_balance as f64 / LAMPORTS_PER_SOL as f64,
            children.len(),
            total_needed as f64 / LAMPORTS_PER_SOL as f64
        ));
    }

    for (i, child) in children.iter().enumerate() {
        let ix = system_instruction::transfer(
            &master.pubkey(),
            &child.pubkey(),
            CHILD_FUNDING_AMOUNT,
        );
        let recent_blockhash = rpc.get_latest_blockhash()?;
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&master.pubkey()),
            &[master],
            recent_blockhash,
        );
        let sig = rpc.send_and_confirm_transaction(&tx)?;
        println!(
            "   Funded child {}/{}: {} -> {} SOL (sig: {})",
            i + 1,
            children.len(),
            child.pubkey(),
            CHILD_FUNDING_AMOUNT as f64 / LAMPORTS_PER_SOL as f64,
            sig
        );
    }

    Ok(())
}

/// Reclaim surplus SOL from child wallets back to master.
pub async fn reclaim_surplus(
    rpc: &RpcClient,
    master: &Keypair,
    children: &[Keypair],
) -> anyhow::Result<()> {
    let mut total_reclaimed = 0u64;

    for (i, child) in children.iter().enumerate() {
        let balance = match rpc.get_balance(&child.pubkey()) {
            Ok(b) if b > 10_000 => b,
            _ => continue,
        };
        let reclaim_amount = balance.saturating_sub(10_000);
        if reclaim_amount == 0 {
            continue;
        }

        let ix = system_instruction::transfer(&child.pubkey(), &master.pubkey(), reclaim_amount);
        let recent_blockhash = rpc.get_latest_blockhash()?;
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&child.pubkey()),
            &[child],
            recent_blockhash,
        );

        match rpc.send_and_confirm_transaction(&tx) {
            Ok(sig) => {
                total_reclaimed += reclaim_amount;
                println!(
                    "   Reclaimed child {}/{}: {} lamports (sig: {})",
                    i + 1,
                    children.len(),
                    reclaim_amount,
                    sig
                );
            }
            Err(e) => {
                eprintln!("   Failed to reclaim from child {}: {}", child.pubkey(), e);
            }
        }
    }

    println!(
        "   Total reclaimed: {} SOL",
        total_reclaimed as f64 / LAMPORTS_PER_SOL as f64
    );
    Ok(())
}
