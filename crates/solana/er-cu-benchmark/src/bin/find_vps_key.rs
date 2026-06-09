use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};
use std::fs;

fn main() {
    let vps_bytes: [u8; 32] = [
        0x5a, 0x71, 0x3c, 0xfd, 0x8c, 0x64, 0x5f, 0x7c,
        0xab, 0xe1, 0xdc, 0x3b, 0x21, 0x87, 0xfd, 0x85,
        0x88, 0xb8, 0x65, 0xa2, 0x27, 0x3e, 0x62, 0x4f,
        0x54, 0x03, 0xb2, 0x9b, 0x16, 0x55, 0xe3, 0x51,
    ];
    let vps = Pubkey::new_from_array(vps_bytes);
    println!("Looking for keypair matching VPS_AUTHORITY={}", vps);

    for entry in fs::read_dir("keys").unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let data = match fs::read_to_string(&path) {
            Ok(d) => d,
            Err(_) => continue,
        };
        let bytes: Vec<u8> = match serde_json::from_str(&data) {
            Ok(b) => b,
            Err(_) => continue,
        };
        if bytes.len() != 64 {
            continue;
        }
        let kp = match Keypair::from_bytes(&bytes) {
            Ok(k) => k,
            Err(_) => continue,
        };
        if kp.pubkey() == vps {
            println!("FOUND MATCH: {}", path.display());
            return;
        }
    }
    println!("No matching keypair found in keys/");
}
