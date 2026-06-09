use solana_client::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::signature::{Keypair, Signer};
use std::fs;

fn main() {
    let rpc = RpcClient::new_with_commitment(
        "https://api.devnet.solana.com".to_string(),
        CommitmentConfig::confirmed(),
    );
    let data = fs::read_to_string("keys/er-cu-children.json").unwrap();
    let arr: Vec<Vec<u8>> = serde_json::from_str(&data).unwrap();
    let mut total = 0u64;
    let mut count = 0usize;
    for bytes in &arr {
        if bytes.len() != 64 { continue; }
        let kp = Keypair::from_bytes(bytes).unwrap();
        let bal = rpc.get_balance(&kp.pubkey()).unwrap_or(0);
        if bal > 0 {
            total += bal;
            count += 1;
        }
    }
    println!("Found {} children with balance, total: {} lamports ({} SOL)", count, total, total as f64 / 1_000_000_000.0);
}
