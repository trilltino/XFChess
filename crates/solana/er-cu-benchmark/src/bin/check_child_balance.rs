use solana_client::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::signature::{Keypair, Signer};
use std::fs;

fn main() {
    let rpc = RpcClient::new_with_commitment(
        "https://api.devnet.solana.com".to_string(),
        CommitmentConfig::confirmed(),
    );
    let children_data = fs::read_to_string("keys/er-cu-children.json").unwrap();
    let children_arr: Vec<Vec<u8>> = serde_json::from_str(&children_data).unwrap();
    let player = Keypair::from_bytes(&children_arr[0]).unwrap();
    let bal = rpc.get_balance(&player.pubkey()).unwrap_or(0);
    println!(
        "Child 0 {}: {} lamports ({} SOL)",
        player.pubkey(),
        bal,
        bal as f64 / 1_000_000_000.0
    );
}
