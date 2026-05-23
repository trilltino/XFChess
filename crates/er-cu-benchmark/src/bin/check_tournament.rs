use solana_client::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;

fn main() {
    let rpc = RpcClient::new_with_commitment(
        "https://api.devnet.solana.com".to_string(),
        CommitmentConfig::confirmed(),
    );
    let program_id: Pubkey = "8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU".parse().unwrap();
    let tournament_id = 1779104020u64;
    let tournament_pda = Pubkey::find_program_address(&[b"tournament", &tournament_id.to_le_bytes()], &program_id).0;
    println!("Tournament PDA: {}", tournament_pda);
    match rpc.get_account(&tournament_pda) {
        Ok(acc) => println!("Exists: {} bytes, {} lamports", acc.data.len(), acc.lamports),
        Err(e) => println!("Error: {}", e),
    }
}
