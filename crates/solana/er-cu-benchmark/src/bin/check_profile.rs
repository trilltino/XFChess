use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};
use std::fs;

fn main() {
    let rpc = RpcClient::new_with_commitment(
        "https://api.devnet.solana.com".to_string(),
        CommitmentConfig::confirmed(),
    );
    let program_id: Pubkey = "8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU"
        .parse()
        .unwrap();
    let children_data = fs::read_to_string("keys/er-cu-children.json").unwrap();
    let children_arr: Vec<Vec<u8>> = serde_json::from_str(&children_data).unwrap();
    let player = Keypair::try_from(children_arr[0].as_slice()).unwrap();

    let profile_pda =
        Pubkey::find_program_address(&[b"profile", player.pubkey().as_ref()], &program_id).0;
    println!("Player: {}", player.pubkey());
    println!("Profile PDA: {}", profile_pda);
    match rpc.get_account(&profile_pda) {
        Ok(acc) => println!(
            "Exists: {} bytes, {} lamports, owner: {}",
            acc.data.len(),
            acc.lamports,
            acc.owner
        ),
        Err(e) => println!("Does not exist: {}", e),
    }
}
