//! Standard wire-format boundary between the latest SDK and stable Agave RPC.
//! No older SDK pin, private-key conversion, or consensus behavior is introduced.
use anyhow::Context as _;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use solana_rpc_client::rpc_client::SerializableTransaction;
use solana_sdk::transaction::{Transaction, VersionedTransaction};
pub use solana_transaction_status_client_types as transaction_status;
use solana_transaction_status_client_types::{EncodedTransaction, TransactionBinaryEncoding};
pub use wincode;
mod rpc;
pub use rpc::{AsyncRpcSdkExt, RpcSdkExt};
pub mod prelude {
    pub use crate::{AsyncRpcSdkExt, RpcSdkExt};
}

pub trait SdkTransaction: Sync {
    fn encoded(&self) -> anyhow::Result<EncodedTransaction>;
}
impl SdkTransaction for Transaction {
    fn encoded(&self) -> anyhow::Result<EncodedTransaction> {
        encoded_transaction(self)
    }
}
impl SdkTransaction for VersionedTransaction {
    fn encoded(&self) -> anyhow::Result<EncodedTransaction> {
        Ok(EncodedTransaction::Binary(
            STANDARD.encode(wincode::serialize(self)?),
            TransactionBinaryEncoding::Base64,
        ))
    }
}

fn provider_transaction(
    transaction: &impl SdkTransaction,
) -> anyhow::Result<impl SerializableTransaction> {
    transaction
        .encoded()?
        .decode()
        .context("Agave rejected SDK transaction encoding")
}

pub fn encoded_transaction(transaction: &Transaction) -> anyhow::Result<EncodedTransaction> {
    Ok(EncodedTransaction::Binary(
        STANDARD.encode(wincode::serialize(transaction)?),
        TransactionBinaryEncoding::Base64,
    ))
}

pub fn rpc_transaction(transaction: &Transaction) -> anyhow::Result<impl SerializableTransaction> {
    encoded_transaction(transaction)?
        .decode()
        .context("Agave rejected the SDK transaction wire format")
}

pub fn rpc_versioned_transaction(
    transaction: &VersionedTransaction,
) -> anyhow::Result<impl SerializableTransaction> {
    EncodedTransaction::Binary(
        STANDARD.encode(wincode::serialize(transaction)?),
        TransactionBinaryEncoding::Base64,
    )
    .decode()
    .context("Agave rejected the SDK versioned transaction wire format")
}

/// Decode our Base64 RPC response into SDK 5 after Agave validates its wire format.
pub fn sdk_decoded_transaction(
    encoded: &EncodedTransaction,
) -> anyhow::Result<VersionedTransaction> {
    anyhow::ensure!(
        encoded.decode().is_some(),
        "Agave rejected RPC transaction encoding"
    );
    let EncodedTransaction::Binary(bytes, TransactionBinaryEncoding::Base64) = encoded else {
        anyhow::bail!("expected Base64 transaction response");
    };
    Ok(wincode::deserialize(&STANDARD.decode(bytes)?)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::{
        hash::Hash,
        instruction::{AccountMeta, Instruction},
        message::{v0, VersionedMessage},
        pubkey::Pubkey,
        signature::{Keypair, Signer},
    };

    fn instruction(payer: Pubkey) -> Instruction {
        Instruction {
            program_id: Pubkey::new_unique(),
            accounts: vec![AccountMeta::new(payer, true)],
            data: vec![0, 1, 127, 128, 255],
        }
    }

    #[test]
    fn legacy_wire_boundary_preserves_signed_message() -> anyhow::Result<()> {
        let payer = Keypair::new();
        let transaction = Transaction::new_signed_with_payer(
            &[instruction(payer.pubkey())],
            Some(&payer.pubkey()),
            &[&payer],
            Hash::new_unique(),
        );
        let decoded = encoded_transaction(&transaction)?
            .decode()
            .context("decode")?;
        let sdk_decoded = sdk_decoded_transaction(&encoded_transaction(&transaction)?)?;
        assert_eq!(sdk_decoded.signatures, transaction.signatures);
        assert_eq!(
            sdk_decoded.message.serialize(),
            transaction.message.serialize()
        );
        assert_eq!(decoded.signatures, transaction.signatures);
        assert_eq!(decoded.message.serialize(), transaction.message.serialize());
        assert!(decoded.signatures[0].verify(payer.pubkey().as_ref(), &decoded.message.serialize()));
        Ok(())
    }

    #[test]
    fn versioned_wire_boundary_preserves_signature_and_message() -> anyhow::Result<()> {
        let payer = Keypair::new();
        let message = v0::Message::try_compile(
            &payer.pubkey(),
            &[instruction(payer.pubkey())],
            &[],
            Hash::new_unique(),
        )?;
        let transaction = VersionedTransaction::try_new(VersionedMessage::V0(message), &[&payer])?;
        let sdk_decoded = sdk_decoded_transaction(&transaction.encoded()?)?;
        assert_eq!(sdk_decoded, transaction);
        let provider = rpc_versioned_transaction(&transaction)?;
        assert_eq!(provider.get_signature(), &transaction.signatures[0]);
        assert_eq!(
            provider.get_recent_blockhash(),
            transaction.message.recent_blockhash()
        );
        assert!(provider
            .get_signature()
            .verify(payer.pubkey().as_ref(), &transaction.message.serialize()));
        Ok(())
    }

    #[test]
    fn malformed_signature_header_is_rejected() -> anyhow::Result<()> {
        let payer = Keypair::new();
        let mut transaction = Transaction::new_signed_with_payer(
            &[instruction(payer.pubkey())],
            Some(&payer.pubkey()),
            &[&payer],
            Hash::new_unique(),
        );
        transaction.signatures.clear();
        assert!(rpc_transaction(&transaction).is_err());
        assert!(sdk_decoded_transaction(&encoded_transaction(&transaction)?).is_err());
        Ok(())
    }
}
