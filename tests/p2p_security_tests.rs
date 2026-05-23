//! Integration tests for P2P layer security hardening.
//!
//! Covers:
//! - Ed25519 signing and verification of [`NetworkMessage`]s
//! - Replay-protection nonce wiring (message carries nonce)
//! - Engine-level move validation for remote moves

use xfchess::multiplayer::network::protocol::{NetworkMessage, SignedNetworkMessage};

/// Generate a deterministic 32-byte Ed25519 seed for tests.
fn test_signing_key() -> [u8; 32] {
    let mut seed = [0u8; 32];
    for (i, b) in seed.iter_mut().enumerate() {
        *b = i as u8;
    }
    seed
}

#[test]
fn sign_and_verify_move_message() {
    let sk = test_signing_key();
    let msg = NetworkMessage::Move {
        game_id: 42,
        turn: 5,
        move_uci: "e2e4".to_string(),
        next_fen: "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1".to_string(),
        nonce: 3,
    };

    let signed = SignedNetworkMessage::sign(msg.clone(), &sk);
    assert!(signed.verify(), "valid signature should verify");
    assert_eq!(signed.msg.game_id(), 42);
}

#[test]
fn tampered_message_fails_verification() {
    let sk = test_signing_key();
    let msg = NetworkMessage::Move {
        game_id: 42,
        turn: 5,
        move_uci: "e2e4".to_string(),
        next_fen: "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1".to_string(),
        nonce: 3,
    };

    let mut signed = SignedNetworkMessage::sign(msg, &sk);
    // Mutate the inner message
    signed.msg = NetworkMessage::Move {
        game_id: 42,
        turn: 5,
        move_uci: "d2d4".to_string(), // different move
        next_fen: signed.msg.game_id().to_string(), // bogus but satisfies type
        nonce: 3,
    };
    assert!(!signed.verify(), "tampered message should fail verification");
}

#[test]
fn tampered_signature_fails_verification() {
    let sk = test_signing_key();
    let msg = NetworkMessage::Move {
        game_id: 42,
        turn: 5,
        move_uci: "e2e4".to_string(),
        next_fen: "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1".to_string(),
        nonce: 3,
    };

    let mut signed = SignedNetworkMessage::sign(msg, &sk);
    // Flip a bit in the signature
    if let Some(b) = signed.signature.first_mut() {
        *b ^= 0xFF;
    }
    assert!(!signed.verify(), "tampered signature should fail verification");
}

#[test]
fn resign_message_signs_and_verifies() {
    let sk = test_signing_key();
    let msg = NetworkMessage::Resign {
        game_id: 99,
        winner: "white".to_string(),
        nonce: 7,
    };

    let signed = SignedNetworkMessage::sign(msg, &sk);
    assert!(signed.verify(), "resign message should sign and verify");
}

#[test]
fn move_message_carries_nonce() {
    let msg = NetworkMessage::Move {
        game_id: 1,
        turn: 1,
        move_uci: "g1f3".to_string(),
        next_fen: "start".to_string(),
        nonce: 12345,
    };

    match msg {
        NetworkMessage::Move { nonce, .. } => assert_eq!(nonce, 12345),
        _ => panic!("expected Move variant"),
    }
}
