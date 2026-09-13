use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::signature::{Keypair, Signer};
use std::fs;

fn main() {
    let rpc = RpcClient::new_with_commitment(
        "https://api.devnet.solana.com".to_string(),
        CommitmentConfig::confirmed(),
    );

    let deployer: Vec<u8> =
        serde_json::from_str(&fs::read_to_string("keys/program-authority.json").unwrap()).unwrap();
    let deployer_kp = Keypair::try_from(deployer.as_slice()).unwrap();
    println!(
        "Deployer {}: {:?}",
        deployer_kp.pubkey(),
        rpc.get_balance(&deployer_kp.pubkey())
    );

    let old_master: Vec<u8> =
        serde_json::from_str(&fs::read_to_string("keys/er-cu-master.json").unwrap()).unwrap();
    let old_master_kp = Keypair::try_from(old_master.as_slice()).unwrap();
    println!(
        "Old master {}: {:?}",
        old_master_kp.pubkey(),
        rpc.get_balance(&old_master_kp.pubkey())
    );

    let fee_payer: Vec<u8> =
        serde_json::from_str(&fs::read_to_string("keys/fee-payer.json").unwrap()).unwrap();
    let fee_payer_kp = Keypair::try_from(fee_payer.as_slice()).unwrap();
    println!(
        "Fee payer {}: {:?}",
        fee_payer_kp.pubkey(),
        rpc.get_balance(&fee_payer_kp.pubkey())
    );

    let temp_fund: Vec<u8> =
        serde_json::from_str(&fs::read_to_string("keys/temp-fund.json").unwrap()).unwrap();
    let temp_fund_kp = Keypair::try_from(temp_fund.as_slice()).unwrap();
    println!(
        "Temp fund {}: {:?}",
        temp_fund_kp.pubkey(),
        rpc.get_balance(&temp_fund_kp.pubkey())
    );
}
