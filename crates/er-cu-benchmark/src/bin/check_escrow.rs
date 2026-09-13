use solana_client::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;

fn main() {
    let rpc = RpcClient::new_with_commitment(
        "https://api.devnet.solana.com".to_string(),
        CommitmentConfig::confirmed(),
    );
    let program_id: Pubkey = "8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU"
        .parse()
        .unwrap();
    let tournament_id = 1779105119u64; // current run's tournament id

    let escrow =
        Pubkey::find_program_address(&[b"t_escrow", &tournament_id.to_le_bytes()], &program_id).0;
    println!("Escrow: {}", escrow);
    match rpc.get_account(&escrow) {
        Ok(acc) => println!(
            "Exists: {} bytes, {} lamports",
            acc.data.len(),
            acc.lamports
        ),
        Err(e) => println!("Does not exist: {}", e),
    }
}
