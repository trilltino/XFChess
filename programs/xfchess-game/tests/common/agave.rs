//! Test-only boundary for SDK 5 values and stable Agave 4.3's Rust API.
//! Transactions cross through its public wire decoder. No dependency is
//! downgraded, and failed conversions fail the test rather than skip execution.
use anchor_lang::solana_program::sysvar::SysvarId;
use serde::{de::DeserializeOwned, Serialize};
use solana_sdk::{
    account::{Account, AccountSharedData},
    hash::Hash,
    pubkey::Pubkey,
    signature::Keypair,
    transaction::{Transaction, TransactionError},
};
use std::ops::{Deref, DerefMut};
use xfchess_client_compat::{encoded_transaction, wincode};

pub use solana_program_test::ProgramTestBanksClientExt;

fn convert<T: DeserializeOwned>(value: impl Serialize) -> T {
    match serde_json::to_value(value).and_then(serde_json::from_value) {
        Ok(value) => value,
        Err(error) => panic!("SDK/Agave fixture conversion failed: {error}"),
    }
}

#[derive(Debug)]
pub enum BanksClientError {
    TransactionError(TransactionError),
    SimulationError { err: TransactionError },
    Other(solana_program_test::BanksClientError),
    Wire(anyhow::Error),
}

impl std::fmt::Display for BanksClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for BanksClientError {}

impl From<solana_program_test::BanksClientError> for BanksClientError {
    fn from(value: solana_program_test::BanksClientError) -> Self {
        match value {
            solana_program_test::BanksClientError::TransactionError(error) => {
                Self::TransactionError(convert(error))
            }
            solana_program_test::BanksClientError::SimulationError { err, .. } => {
                Self::SimulationError { err: convert(err) }
            }
            other => Self::Other(other),
        }
    }
}

pub struct MetadataResult {
    pub result: Result<(), TransactionError>,
}

pub struct BanksClient(solana_program_test::BanksClient);

impl Deref for BanksClient {
    type Target = solana_program_test::BanksClient;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for BanksClient {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl BanksClient {
    pub async fn process_transaction(
        &self,
        transaction: Transaction,
    ) -> Result<(), BanksClientError> {
        let encoded = encoded_transaction(&transaction).map_err(BanksClientError::Wire)?;
        let decoded = encoded.decode().ok_or_else(|| {
            BanksClientError::Wire(anyhow::anyhow!("Agave rejected SDK transaction encoding"))
        })?;
        self.0
            .process_transaction(decoded)
            .await
            .map_err(Into::into)
    }

    pub async fn process_transaction_with_metadata(
        &self,
        transaction: Transaction,
    ) -> Result<MetadataResult, BanksClientError> {
        let encoded = encoded_transaction(&transaction).map_err(BanksClientError::Wire)?;
        let decoded = encoded.decode().ok_or_else(|| {
            BanksClientError::Wire(anyhow::anyhow!("Agave rejected SDK transaction encoding"))
        })?;
        let result = self.0.process_transaction_with_metadata(decoded).await?;
        Ok(MetadataResult {
            result: result.result.map_err(convert),
        })
    }

    pub async fn get_account(&self, address: Pubkey) -> Result<Option<Account>, BanksClientError> {
        Ok(self.0.get_account(address).await?.map(|value| Account {
            lamports: value.lamports,
            data: value.data,
            owner: value.owner,
            executable: value.executable,
            rent_epoch: value.rent_epoch,
        }))
    }

    pub async fn get_sysvar<T>(&self) -> Result<T, BanksClientError>
    where
        T: wincode::DeserializeOwned<Dst = T> + SysvarId,
    {
        let account = self
            .get_account(T::id())
            .await?
            .ok_or_else(|| BanksClientError::Wire(anyhow::anyhow!("missing fixture sysvar")))?;
        wincode::deserialize(&account.data).map_err(|error| BanksClientError::Wire(error.into()))
    }
}

pub struct ProgramTest(solana_program_test::ProgramTest);
impl Deref for ProgramTest {
    type Target = solana_program_test::ProgramTest;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for ProgramTest {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl ProgramTest {
    pub fn new(name: &'static str, program: Pubkey, processor: Option<()>) -> Self {
        assert!(
            processor.is_none(),
            "SBF regression tests must load the built program"
        );
        Self(solana_program_test::ProgramTest::new(name, program, None))
    }
    pub fn add_account(&mut self, address: Pubkey, account: Account) {
        self.0.add_account(address, convert(account));
    }
    pub async fn start_with_context(self) -> ProgramTestContext {
        let inner = self.0.start_with_context().await;
        // ProgramTest generated this local fixture key. It is never loaded
        // from a deployed authority file, logged or persisted.
        let payer = match Keypair::try_from(inner.payer.to_bytes().as_slice()) {
            Ok(payer) => payer,
            Err(error) => panic!("local test payer conversion failed: {error}"),
        };
        ProgramTestContext {
            banks_client: BanksClient(inner.banks_client.clone()),
            last_blockhash: inner.last_blockhash,
            payer,
            inner,
        }
    }
}

pub struct ProgramTestContext {
    pub banks_client: BanksClient,
    pub last_blockhash: Hash,
    pub payer: Keypair,
    inner: solana_program_test::ProgramTestContext,
}
impl Deref for ProgramTestContext {
    type Target = solana_program_test::ProgramTestContext;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl DerefMut for ProgramTestContext {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
impl ProgramTestContext {
    pub fn warp_to_slot(&mut self, slot: u64) -> Result<(), solana_program_test::ProgramTestError> {
        self.inner.warp_to_slot(slot)?;
        self.last_blockhash = self.inner.last_blockhash;
        Ok(())
    }
    pub fn set_account(&mut self, address: &Pubkey, account: &AccountSharedData) {
        let account: Account = account.clone().into();
        self.inner.set_account(address, &convert(account));
    }
}
