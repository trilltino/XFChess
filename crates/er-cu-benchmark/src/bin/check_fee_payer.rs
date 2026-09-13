use solana_sdk::signature::{Keypair, Signer};
use std::fs;

fn main() {
    let data: Vec<u8> =
        serde_json::from_str(&fs::read_to_string("keys/fee-payer.json").unwrap()).unwrap();
    let kp = Keypair::try_from(data.as_slice()).unwrap();
    println!("FEE_PAYER_PUBKEY={}", kp.pubkey());
}
