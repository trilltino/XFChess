#![cfg(feature = "solana")]
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

const PROGRAM_ID: &str = "8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU";

fn main() {
    let wallet_arg = std::env::args()
        .nth(1)
        .expect("Usage: profile_pda <wallet_pubkey>");

    let wallet_pubkey = Pubkey::from_str(&wallet_arg)
        .expect("Invalid wallet public key");

    let program_id = Pubkey::from_str(PROGRAM_ID).unwrap();

    let (profile_pda, profile_bump) =
        Pubkey::find_program_address(&[b"profile", wallet_pubkey.as_ref()], &program_id);

    println!("Wallet Pubkey: {}", wallet_pubkey);
    println!("Program ID   : {}", program_id);
    println!();
    println!("Profile PDA   : {} (bump {})", profile_pda, profile_bump);
    println!();
    println!("Solscan (devnet):");
    println!("  https://solscan.io/account/{}?cluster=devnet", profile_pda);
    println!();
    println!("MagicBlock Explorer:");
    println!("  https://explorer.magicblock.app/account/{}", profile_pda);
}
