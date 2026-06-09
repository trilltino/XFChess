use solana_client::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::system_instruction;
use solana_sdk::transaction::Transaction;
use std::fs;

fn main() {
    let rpc = RpcClient::new_with_commitment(
        "https://api.devnet.solana.com".to_string(),
        CommitmentConfig::confirmed(),
    );

    let deployer_bytes: Vec<u8> = serde_json::from_str(&fs::read_to_string("keys/program-authority.json").unwrap()).unwrap();
    let deployer = Keypair::from_bytes(&deployer_bytes).unwrap();
    println!("Deployer: {}", deployer.pubkey());

    // Reclaim from old master
    let old_master_bytes: Vec<u8> = serde_json::from_str(&fs::read_to_string("keys/er-cu-master.json").unwrap()).unwrap();
    let old_master = Keypair::from_bytes(&old_master_bytes).unwrap();
    let old_master_bal = rpc.get_balance(&old_master.pubkey()).unwrap_or(0);
    println!("Old master balance: {}", old_master_bal);
    if old_master_bal > 1_000_000 {
        let ix = system_instruction::transfer(&old_master.pubkey(), &deployer.pubkey(), old_master_bal - 1_000_000);
        let bh = rpc.get_latest_blockhash().unwrap();
        let tx = Transaction::new_signed_with_payer(&[ix], Some(&old_master.pubkey()), &[&old_master], bh);
        match rpc.send_and_confirm_transaction(&tx) {
            Ok(sig) => println!("Transferred from old master: {}", sig),
            Err(e) => println!("Failed to transfer from old master: {}", e),
        }
    }

    // Reclaim from children
    let children_data = fs::read_to_string("keys/er-cu-children.json").unwrap();
    let children_arr: Vec<Vec<u8>> = serde_json::from_str(&children_data).unwrap();
    let children: Vec<Keypair> = children_arr.iter().filter_map(|b| Keypair::from_bytes(b).ok()).collect();

    let mut total_reclaimed = 0u64;
    for (i, child) in children.iter().enumerate() {
        let bal = rpc.get_balance(&child.pubkey()).unwrap_or(0);
        if bal > 1_000_000 {
            let amount = bal - 1_000_000;
            let ix = system_instruction::transfer(&child.pubkey(), &deployer.pubkey(), amount);
            let bh = rpc.get_latest_blockhash().unwrap();
            let tx = Transaction::new_signed_with_payer(&[ix], Some(&child.pubkey()), &[child], bh);
            match rpc.send_and_confirm_transaction(&tx) {
                Ok(sig) => {
                    total_reclaimed += amount;
                    println!("Child {}: reclaimed {} lamports ({})", i, amount, sig);
                }
                Err(e) => println!("Child {}: failed {}", i, e),
            }
        }
    }

    let deployer_bal = rpc.get_balance(&deployer.pubkey()).unwrap_or(0);
    println!("Deployer final balance: {} SOL", deployer_bal as f64 / 1_000_000_000.0);
    println!("Total reclaimed from children: {} SOL", total_reclaimed as f64 / 1_000_000_000.0);
}
