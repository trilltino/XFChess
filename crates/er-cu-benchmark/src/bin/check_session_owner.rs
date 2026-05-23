use solana_client::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};
use std::fs;

fn main() {
    let rpc = RpcClient::new_with_commitment(
        "https://api.devnet.solana.com".to_string(),
        CommitmentConfig::confirmed(),
    );
    let program_id: Pubkey = "8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU".parse().unwrap();
    let tournament_id = 1779104020u64;
    let children_data = fs::read_to_string("keys/er-cu-children.json").unwrap();
    let children_arr: Vec<Vec<u8>> = serde_json::from_str(&children_data).unwrap();
    let player = Keypair::from_bytes(&children_arr[0]).unwrap();

    let session_delegation = Pubkey::find_program_address(
        &[b"tournament_session", &tournament_id.to_le_bytes(), player.pubkey().as_ref()],
        &program_id,
    ).0;

    match rpc.get_account(&session_delegation) {
        Ok(acc) => println!("Owner: {}, Data len: {}, Lamports: {}", acc.owner, acc.data.len(), acc.lamports),
        Err(e) => println!("Error: {}", e),
    }
}
