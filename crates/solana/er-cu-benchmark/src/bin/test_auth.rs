use solana_client::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::transaction::Transaction;
use std::fs;
use std::str::FromStr;

fn main() {
    let rpc = RpcClient::new_with_commitment(
        "https://api.devnet.solana.com".to_string(),
        CommitmentConfig::confirmed(),
    );

    let deployer_bytes: Vec<u8> =
        serde_json::from_str(&fs::read_to_string("keys/program-authority.json").unwrap()).unwrap();
    let deployer = Keypair::from_bytes(&deployer_bytes).unwrap();

    let children_data = fs::read_to_string("keys/er-cu-children.json").unwrap();
    let children_arr: Vec<Vec<u8>> = serde_json::from_str(&children_data).unwrap();
    let player = Keypair::from_bytes(&children_arr[0]).unwrap();

    let program_id: Pubkey = "8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU"
        .parse()
        .unwrap();
    let tournament_id = 1779104020u64;

    let session = Keypair::new();
    let session_pubkey = session.pubkey();

    let tournament_pda =
        Pubkey::find_program_address(&[b"tournament", &tournament_id.to_le_bytes()], &program_id).0;
    let shard0 = Pubkey::find_program_address(
        &[b"tourney_players", &[0u8], &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;
    let shard1 = Pubkey::find_program_address(
        &[b"tourney_players", &[1u8], &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;
    let shard2 = Pubkey::find_program_address(
        &[b"tourney_players", &[2u8], &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;
    let shard3 = Pubkey::find_program_address(
        &[b"tourney_players", &[3u8], &tournament_id.to_le_bytes()],
        &program_id,
    )
    .0;
    let session_delegation = Pubkey::find_program_address(
        &[
            b"tournament_session",
            &tournament_id.to_le_bytes(),
            player.pubkey().as_ref(),
        ],
        &program_id,
    )
    .0;

    println!("Player: {}", player.pubkey());
    println!(
        "Player balance: {}",
        rpc.get_balance(&player.pubkey()).unwrap_or(0)
    );
    println!(
        "Tournament PDA: {} ({} lamports)",
        tournament_pda,
        rpc.get_account(&tournament_pda)
            .map(|a| a.lamports)
            .unwrap_or(0)
    );
    println!(
        "Shard0: {} ({} lamports)",
        shard0,
        rpc.get_account(&shard0).map(|a| a.lamports).unwrap_or(0)
    );
    println!(
        "Shard1: {} ({} lamports)",
        shard1,
        rpc.get_account(&shard1).map(|a| a.lamports).unwrap_or(0)
    );
    println!(
        "Shard2: {} ({} lamports)",
        shard2,
        rpc.get_account(&shard2).map(|a| a.lamports).unwrap_or(0)
    );
    println!(
        "Shard3: {} ({} lamports)",
        shard3,
        rpc.get_account(&shard3).map(|a| a.lamports).unwrap_or(0)
    );
    println!(
        "Session delegation: {} (exists: {})",
        session_delegation,
        rpc.get_account(&session_delegation).is_ok()
    );

    let mut data = [0u8; 8];
    data.copy_from_slice(&[0x1a, 0xf6, 0x2c, 0xaf, 0xfd, 0xcf, 0x17, 0x1e]); // authorize_tournament_session discriminator
    data = [0; 8]; // Actually compute it properly... just use hex
                   // anchor_discriminator("authorize_tournament_session")
                   // Let me just hardcode it: sha256("global:authorize_tournament_session")[..8]
}
