use std::fs::File;
use std::io::Read;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut vps_file = File::open("keys/authorities/vps-authority.json")?;
    let mut vps_bytes = String::new();
    vps_file.read_to_string(&mut vps_bytes)?;
    let vps_vec: Vec<u8> = serde_json::from_str(&vps_bytes)?;
    println!("VPS_BASE58: {}", bs58::encode(vps_vec).into_string());

    let mut kyc_file = File::open("keys/authorities/kyc-authority.json")?;
    let mut kyc_bytes = String::new();
    kyc_file.read_to_string(&mut kyc_bytes)?;
    let kyc_vec: Vec<u8> = serde_json::from_str(&kyc_bytes)?;
    println!("KYC_BASE58: {}", bs58::encode(kyc_vec).into_string());

    Ok(())
}
