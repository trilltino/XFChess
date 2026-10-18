//! Verifies that a relayed move was authorised by the player it is recorded
//! for. The signed message is defined in `braid_chess::move_auth`.

use solana_sdk::{pubkey::Pubkey, signature::Signature};
use std::str::FromStr;

/// Whether `signature` (base58) is `signer`'s authorisation of `uci` as ply
/// `ply` of `game_id`, played in reply to `previous_uci`.
pub fn is_authorized(
    signer: &Pubkey,
    signature: &str,
    game_id: u64,
    ply: u64,
    uci: &str,
    previous_uci: Option<&str>,
) -> bool {
    let Ok(signature) = Signature::from_str(signature) else {
        return false;
    };
    let message = braid_chess::move_auth::message(game_id, ply, uci, previous_uci);
    signature.verify(signer.as_ref(), &message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::signature::{Keypair, Signer};

    fn sign(kp: &Keypair, game_id: u64, ply: u64, uci: &str, previous: Option<&str>) -> String {
        kp.sign_message(&braid_chess::move_auth::message(
            game_id, ply, uci, previous,
        ))
        .to_string()
    }

    #[test]
    fn accepts_only_the_signed_move_in_its_context() {
        let kp = Keypair::new();
        let signer = kp.pubkey();
        let sig = sign(&kp, 9, 2, "e7e5", Some("e2e4"));

        assert!(is_authorized(&signer, &sig, 9, 2, "e7e5", Some("e2e4")));
        assert!(!is_authorized(&signer, &sig, 9, 2, "d7d5", Some("e2e4")));
        assert!(!is_authorized(&signer, &sig, 9, 4, "e7e5", Some("e2e4")));
        assert!(!is_authorized(&signer, &sig, 9, 2, "e7e5", Some("d2d4")));
        assert!(!is_authorized(&signer, &sig, 10, 2, "e7e5", Some("e2e4")));
        assert!(!is_authorized(
            &Keypair::new().pubkey(),
            &sig,
            9,
            2,
            "e7e5",
            Some("e2e4")
        ));
    }

    #[test]
    fn malformed_signatures_are_rejected() {
        let signer = Keypair::new().pubkey();
        assert!(!is_authorized(&signer, "", 1, 1, "e2e4", None));
        assert!(!is_authorized(&signer, "not-base58!", 1, 1, "e2e4", None));
    }
}
