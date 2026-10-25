use crate::{provider_transaction, SdkTransaction};
use solana_commitment_config::CommitmentConfig;
use solana_rpc_client_api::{
    client_error::{ErrorKind, Result as ClientResult},
    config::{RpcSendTransactionConfig, RpcSimulateTransactionConfig},
    response::{Response, RpcSimulateTransactionResult},
};
use solana_sdk::{account::Account, pubkey::Pubkey, signature::Signature};

fn wire_error(error: anyhow::Error) -> solana_rpc_client_api::client_error::Error {
    ErrorKind::Custom(error.to_string()).into()
}

pub trait RpcSdkExt {
    fn get_sdk_account(&self, address: &Pubkey) -> ClientResult<Account>;
    fn get_multiple_sdk_accounts(&self, addresses: &[Pubkey])
        -> ClientResult<Vec<Option<Account>>>;
    fn send_sdk_transaction(&self, tx: &impl SdkTransaction) -> ClientResult<Signature>;
    fn send_sdk_transaction_with_config(
        &self,
        tx: &impl SdkTransaction,
        config: RpcSendTransactionConfig,
    ) -> ClientResult<Signature>;
    fn send_sdk_and_confirm_transaction(&self, tx: &impl SdkTransaction)
        -> ClientResult<Signature>;
    fn send_sdk_and_confirm_transaction_with_spinner_and_config(
        &self,
        tx: &impl SdkTransaction,
        commitment: CommitmentConfig,
        config: RpcSendTransactionConfig,
    ) -> ClientResult<Signature>;
    fn simulate_sdk_transaction(
        &self,
        tx: &impl SdkTransaction,
    ) -> ClientResult<Response<RpcSimulateTransactionResult>>;
    fn simulate_sdk_transaction_with_config(
        &self,
        tx: &impl SdkTransaction,
        config: RpcSimulateTransactionConfig,
    ) -> ClientResult<Response<RpcSimulateTransactionResult>>;
}

impl RpcSdkExt for solana_rpc_client::rpc_client::RpcClient {
    fn get_sdk_account(&self, address: &Pubkey) -> ClientResult<Account> {
        self.get_account(address).map(|account| Account {
            lamports: account.lamports,
            data: account.data,
            owner: account.owner,
            executable: account.executable,
            rent_epoch: account.rent_epoch,
        })
    }
    fn get_multiple_sdk_accounts(
        &self,
        addresses: &[Pubkey],
    ) -> ClientResult<Vec<Option<Account>>> {
        self.get_multiple_accounts(addresses).map(|accounts| {
            accounts
                .into_iter()
                .map(|account| {
                    account.map(|account| Account {
                        lamports: account.lamports,
                        data: account.data,
                        owner: account.owner,
                        executable: account.executable,
                        rent_epoch: account.rent_epoch,
                    })
                })
                .collect()
        })
    }
    fn send_sdk_transaction(&self, tx: &impl SdkTransaction) -> ClientResult<Signature> {
        self.send_transaction(&provider_transaction(tx).map_err(wire_error)?)
    }
    fn send_sdk_transaction_with_config(
        &self,
        tx: &impl SdkTransaction,
        config: RpcSendTransactionConfig,
    ) -> ClientResult<Signature> {
        self.send_transaction_with_config(&provider_transaction(tx).map_err(wire_error)?, config)
    }
    fn send_sdk_and_confirm_transaction(
        &self,
        tx: &impl SdkTransaction,
    ) -> ClientResult<Signature> {
        self.send_and_confirm_transaction(&provider_transaction(tx).map_err(wire_error)?)
    }
    fn send_sdk_and_confirm_transaction_with_spinner_and_config(
        &self,
        tx: &impl SdkTransaction,
        commitment: CommitmentConfig,
        config: RpcSendTransactionConfig,
    ) -> ClientResult<Signature> {
        self.send_and_confirm_transaction_with_spinner_and_config(
            &provider_transaction(tx).map_err(wire_error)?,
            commitment,
            config,
        )
    }
    fn simulate_sdk_transaction(
        &self,
        tx: &impl SdkTransaction,
    ) -> ClientResult<Response<RpcSimulateTransactionResult>> {
        self.simulate_transaction(&provider_transaction(tx).map_err(wire_error)?)
    }
    fn simulate_sdk_transaction_with_config(
        &self,
        tx: &impl SdkTransaction,
        config: RpcSimulateTransactionConfig,
    ) -> ClientResult<Response<RpcSimulateTransactionResult>> {
        self.simulate_transaction_with_config(
            &provider_transaction(tx).map_err(wire_error)?,
            config,
        )
    }
}

#[async_trait::async_trait]
pub trait AsyncRpcSdkExt {
    async fn send_sdk_transaction(&self, tx: &impl SdkTransaction) -> ClientResult<Signature>;
    async fn send_sdk_transaction_with_config(
        &self,
        tx: &impl SdkTransaction,
        config: RpcSendTransactionConfig,
    ) -> ClientResult<Signature>;
    async fn send_sdk_and_confirm_transaction(
        &self,
        tx: &impl SdkTransaction,
    ) -> ClientResult<Signature>;
    async fn send_sdk_and_confirm_transaction_with_spinner_and_config(
        &self,
        tx: &impl SdkTransaction,
        commitment: CommitmentConfig,
        config: RpcSendTransactionConfig,
    ) -> ClientResult<Signature>;
    async fn simulate_sdk_transaction(
        &self,
        tx: &impl SdkTransaction,
    ) -> ClientResult<Response<RpcSimulateTransactionResult>>;
    async fn simulate_sdk_transaction_with_config(
        &self,
        tx: &impl SdkTransaction,
        config: RpcSimulateTransactionConfig,
    ) -> ClientResult<Response<RpcSimulateTransactionResult>>;
}

#[async_trait::async_trait]
impl AsyncRpcSdkExt for solana_rpc_client::nonblocking::rpc_client::RpcClient {
    async fn send_sdk_transaction(&self, tx: &impl SdkTransaction) -> ClientResult<Signature> {
        self.send_transaction(&provider_transaction(tx).map_err(wire_error)?)
            .await
    }
    async fn send_sdk_transaction_with_config(
        &self,
        tx: &impl SdkTransaction,
        config: RpcSendTransactionConfig,
    ) -> ClientResult<Signature> {
        self.send_transaction_with_config(&provider_transaction(tx).map_err(wire_error)?, config)
            .await
    }
    async fn send_sdk_and_confirm_transaction(
        &self,
        tx: &impl SdkTransaction,
    ) -> ClientResult<Signature> {
        self.send_and_confirm_transaction(&provider_transaction(tx).map_err(wire_error)?)
            .await
    }
    async fn send_sdk_and_confirm_transaction_with_spinner_and_config(
        &self,
        tx: &impl SdkTransaction,
        commitment: CommitmentConfig,
        config: RpcSendTransactionConfig,
    ) -> ClientResult<Signature> {
        self.send_and_confirm_transaction_with_spinner_and_config(
            &provider_transaction(tx).map_err(wire_error)?,
            commitment,
            config,
        )
        .await
    }
    async fn simulate_sdk_transaction(
        &self,
        tx: &impl SdkTransaction,
    ) -> ClientResult<Response<RpcSimulateTransactionResult>> {
        self.simulate_transaction(&provider_transaction(tx).map_err(wire_error)?)
            .await
    }
    async fn simulate_sdk_transaction_with_config(
        &self,
        tx: &impl SdkTransaction,
        config: RpcSimulateTransactionConfig,
    ) -> ClientResult<Response<RpcSimulateTransactionResult>> {
        self.simulate_transaction_with_config(
            &provider_transaction(tx).map_err(wire_error)?,
            config,
        )
        .await
    }
}
