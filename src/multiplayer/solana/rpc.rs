use bevy::prelude::*;
use solana_sdk::pubkey::Pubkey;

#[derive(Resource)]
pub struct SolanaRpc {
    pub rpc_url: String,
    pub fee_payer: String,
}

impl SolanaRpc {
    pub fn new(rpc_url: String, fee_payer: String) -> Self {
        SolanaRpc {
            rpc_url,
            fee_payer,
        }
    }

    pub fn get_fee_payer(&self) -> Result<Pubkey, String> {
        self.fee_payer.parse::<Pubkey>().map_err(|e| format!("Invalid fee payer pubkey: {}", e))
    }
}
