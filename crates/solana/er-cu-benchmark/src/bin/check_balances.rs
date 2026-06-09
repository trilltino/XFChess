use solana_client::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::signature::{Keypair, Signer};
use std::fs;

fn main() {
    let rpc = RpcClient::new_with_commitment(
        "https://api.devnet.solana.com".to_string(),
        CommitmentConfig::confirmed(),
    );

    let deployer: Vec<u8> = serde_json::from_str(&fs::read_to_string("keys/program-authority.json").unwrap()).unwrap();
    let deployer_kp = Keypair::from_bytes(&deployer).unwrap();
    println!("Deployer {}: {:?}", deployer_kp.pubkey(), rpc.get_balance(&deployer_kp.pubkey()));

    let old_master: Vec<u8> = serde_json::from_str(&fs::read_to_string("keys/er-cu-master.json").unwrap()).unwrap();
    let old_master_kp = Keypair::from_bytes(&old_master).unwrap();
    println!("Old master {}: {:?}", old_master_kp.pubkey(), rpc.get_balance(&old_master_kp.pubkey()));

    let fee_payer: Vec<u8> = serde_json::from_str(&fs::read_to_string("keys/fee-payer.json").unwrap()).unwrap();
    let fee_payer_kp = Keypair::from_bytes(&fee_payer).unwrap();
    println!("Fee payer {}: {:?}", fee_payer_kp.pubkey(), rpc.get_balance(&fee_payer_kp.pubkey()));

    let temp_fund: Vec<u8> = serde_json::from_str(&fs::read_to_string("keys/temp-fund.json").unwrap()).unwrap();
    let temp_fund_kp = Keypair::from_bytes(&temp_fund).unwrap();
    println!("Temp fund {}: {:?}", temp_fund_kp.pubkey(), rpc.get_balance(&temp_fund_kp.pubkey()));
}
