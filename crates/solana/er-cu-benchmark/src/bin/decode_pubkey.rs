use solana_sdk::pubkey::Pubkey;

fn main() {
    let pk: Pubkey = "C1vn2MT7tZotZPjUJQDf9oo3dpZZ2tr7NxYLg8jTYgkw"
        .parse()
        .unwrap();
    let bytes = pk.to_bytes();
    println!("Pubkey: {}", pk);
    println!("Bytes ({}):", bytes.len());
    for (i, b) in bytes.iter().enumerate() {
        if i % 8 == 0 {
            print!("        ");
        }
        print!("0x{:02x}, ", b);
        if i % 8 == 7 {
            println!();
        }
    }
    if bytes.len() % 8 != 0 {
        println!();
    }
}
