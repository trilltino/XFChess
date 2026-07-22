use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};
use std::fs;

fn main() {
    let master_data: Vec<u8> =
        serde_json::from_str(&fs::read_to_string("keys/er-cu-master.json").unwrap()).unwrap();
    let master = Keypair::try_from(master_data.as_slice()).unwrap();
    println!("MASTER_PUBKEY={}", master.pubkey());

    let auth_data: Vec<u8> =
        serde_json::from_str(&fs::read_to_string("keys/program-authority.json").unwrap()).unwrap();
    let auth = Keypair::try_from(auth_data.as_slice()).unwrap();
    println!("PROGRAM_AUTH_PUBKEY={}", auth.pubkey());

    let vps_bytes: [u8; 32] = [
        0x5a, 0x71, 0x3c, 0xfd, 0x8c, 0x64, 0x5f, 0x7c, 0xab, 0xe1, 0xdc, 0x3b, 0x21, 0x87, 0xfd,
        0x85, 0x88, 0xb8, 0x65, 0xa2, 0x27, 0x3e, 0x62, 0x4f, 0x54, 0x03, 0xb2, 0x9b, 0x16, 0x55,
        0xe3, 0x51,
    ];
    let vps = Pubkey::new_from_array(vps_bytes);
    println!("VPS_AUTHORITY_PUBKEY={}", vps);
}
