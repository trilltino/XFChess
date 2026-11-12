use async_trait::async_trait;
use solana_sdk::pubkey::Pubkey;
use xfchess_platform_api::{WalletSignRequest, WalletSignResponse};

#[async_trait]
pub trait WalletSigner: Send + Sync {
    fn public_key(&self) -> Option<Pubkey>;
    async fn sign(&self, request: WalletSignRequest) -> WalletSignResponse;
}
