use solana_sdk::signature::{Keypair, Signer};
use std::fs;

fn main() {
    let path = format!(
        "{}/.config/solana/id.json",
        std::env::var("USERPROFILE").unwrap()
    );
    let data: Vec<u8> = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    let kp = Keypair::try_from(data.as_slice()).unwrap();
    println!("ID_JSON_PUBKEY={}", kp.pubkey());
    let bytes = kp.pubkey().to_bytes();
    println!("ID_JSON_BYTES=[");
    for (i, b) in bytes.iter().enumerate() {
        if i % 8 == 0 {
            print!("        ");
        }
        print!("0x{:02x}, ", b);
        if i % 8 == 7 {
            println!();
        }
    }
    println!("    ]");
}
