use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

const PROGRAM_ID: &str = "FVPp29xDtMrh3CrTJNnxDcbGRnMMKuUv2ntqkBRc1uDX";

fn main() {
    let game_id: u64 = std::env::args()
        .nth(1)
        .expect("Usage: pda <game_id>")
        .parse()
        .expect("game_id must be a u64");

    let program_id = Pubkey::from_str(PROGRAM_ID).unwrap();
    let id_bytes = game_id.to_le_bytes();

    let (game_pda, game_bump) =
        Pubkey::find_program_address(&[b"game", &id_bytes], &program_id);

    let (move_log_pda, log_bump) =
        Pubkey::find_program_address(&[b"move_log", &id_bytes], &program_id);

    let (session_del_pda, sess_bump) =
        Pubkey::find_program_address(&[b"session_delegation", &id_bytes], &program_id);

    println!("Game ID : {}", game_id);
    println!("Program : {}", program_id);
    println!();
    println!("game PDA              : {} (bump {})", game_pda, game_bump);
    println!("move_log PDA          : {} (bump {})", move_log_pda, log_bump);
    println!("session_delegation PDA: {} (bump {})", session_del_pda, sess_bump);
    println!();
    println!("Solscan (devnet):");
    println!("  https://solscan.io/account/{}?cluster=devnet", game_pda);
    println!("  https://solscan.io/account/{}?cluster=devnet", move_log_pda);
    println!();
    println!("MagicBlock Explorer:");
    println!("  https://explorer.magicblock.app/account/{}", game_pda);
    println!("  https://explorer.magicblock.app/account/{}", move_log_pda);
}
