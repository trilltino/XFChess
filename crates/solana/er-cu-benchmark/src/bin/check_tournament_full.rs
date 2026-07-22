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
    let tournament_id = 1779118602u64;

    let tournament_pda =
        Pubkey::find_program_address(&[b"tournament", &tournament_id.to_le_bytes()], &program_id).0;
    println!("Tournament PDA: {}", tournament_pda);
    match rpc.get_account(&tournament_pda) {
        Ok(acc) => println!(
            "  EXISTS: {} bytes, {} lamports",
            acc.data.len(),
            acc.lamports
        ),
        Err(e) => println!("  MISSING: {}", e),
    }

    for i in 0..4 {
        let shard = Pubkey::find_program_address(
            &[b"tourney_players", &[i], &tournament_id.to_le_bytes()],
            &program_id,
        )
        .0;
        println!("Shard {}: {}", i, shard);
        match rpc.get_account(&shard) {
            Ok(acc) => println!(
                "  EXISTS: {} bytes, {} lamports",
                acc.data.len(),
                acc.lamports
            ),
            Err(e) => println!("  MISSING: {}", e),
        }
    }

    let escrow =
        Pubkey::find_program_address(&[b"t_escrow", &tournament_id.to_le_bytes()], &program_id).0;
    println!("Escrow: {}", escrow);
    match rpc.get_account(&escrow) {
        Ok(acc) => println!(
            "  EXISTS: {} bytes, {} lamports",
            acc.data.len(),
            acc.lamports
        ),
        Err(e) => println!("  MISSING: {}", e),
    }
}
