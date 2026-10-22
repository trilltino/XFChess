//! Signs this player's own moves so the other participant can relay them to
//! the backend's `record_move` (message format: `braid_chess::move_auth`).
//!
//! The key is this device's persistent per-wallet gossip seed
//! (`device_id::wallet_gossip_seed`), so no wallet prompt is involved. The seat
//! claim registers its public key as the seat's `move_signer`; the backend
//! refuses a relayed move without a matching signature.

use ed25519_dalek::{Signer, SigningKey};

fn device_key(wallet: &str) -> SigningKey {
    SigningKey::from_bytes(&super::device_id::wallet_gossip_seed(wallet))
}

/// This device's move-signing public key for `wallet`, base58.
pub fn signer_pubkey(wallet: &str) -> String {
    bs58::encode(device_key(wallet).verifying_key().to_bytes()).into_string()
}

/// `wallet`'s authorisation of `uci`, the move that produced `fen_after`, in
/// reply to `previous_uci`. `None` when `fen_after` names no ply.
pub fn sign_move(
    wallet: &str,
    game_id: u64,
    uci: &str,
    fen_after: &str,
    previous_uci: Option<&str>,
) -> Option<String> {
    sign_with(&device_key(wallet), game_id, uci, fen_after, previous_uci)
}

fn sign_with(
    key: &SigningKey,
    game_id: u64,
    uci: &str,
    fen_after: &str,
    previous_uci: Option<&str>,
) -> Option<String> {
    // The on-chain move nonce is the ply, counted from the standard start.
    let ply = crate::multiplayer::systems::fen_ply(fen_after)?;
    let message = braid_chess::move_auth::message(game_id, ply, uci, previous_uci);
    Some(bs58::encode(key.sign(&message).to_bytes()).into_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signature, Verifier};

    #[test]
    fn signature_covers_the_ply_named_by_the_resulting_position(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let key = SigningKey::from_bytes(&[7u8; 32]);
        let after_e5 = "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e6 0 2";
        let sig = sign_with(&key, 42, "e7e5", after_e5, Some("e2e4"))
            .ok_or("invalid signing position")?;

        let bytes: [u8; 64] = bs58::decode(sig)
            .into_vec()?
            .try_into()
            .map_err(|_| "signature must have 64 bytes")?;
        let signature = Signature::from_bytes(&bytes);
        let verify = |ply, previous| {
            key.verifying_key()
                .verify(
                    &braid_chess::move_auth::message(42, ply, "e7e5", previous),
                    &signature,
                )
                .is_ok()
        };
        assert!(verify(2, Some("e2e4")));
        assert!(!verify(3, Some("e2e4")));
        assert!(!verify(2, Some("d2d4")));

        Ok(())
    }

    /// The backend verifies with `solana_sdk`; the registered key and the
    /// signature must parse and verify there exactly as the client encodes them.
    #[cfg(feature = "solana")]
    #[test]
    fn the_backends_verifier_accepts_the_clients_signature(
    ) -> Result<(), Box<dyn std::error::Error>> {
        use solana_sdk::{pubkey::Pubkey, signature::Signature};
        use std::str::FromStr;

        let key = SigningKey::from_bytes(&[11u8; 32]);
        let signer = Pubkey::from_str(&bs58::encode(key.verifying_key().to_bytes()).into_string())?;
        let after_e4 = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1";
        let sig = sign_with(&key, 5, "e2e4", after_e4, None).ok_or("invalid signing position")?;

        let signature = Signature::from_str(&sig)?;
        let message = braid_chess::move_auth::message(5, 1, "e2e4", None);
        assert!(signature.verify(signer.as_ref(), &message));
        let other_ply = braid_chess::move_auth::message(5, 3, "e2e4", None);
        assert!(!signature.verify(signer.as_ref(), &other_ply));

        Ok(())
    }

    #[test]
    fn a_position_without_a_ply_is_not_signed() {
        let key = SigningKey::from_bytes(&[7u8; 32]);
        assert_eq!(sign_with(&key, 1, "e2e4", "not a fen", None), None);
    }
}
