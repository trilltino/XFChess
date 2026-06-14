//! Wire + signing round-trips for the P2P `NetworkMessage`.
//!
//! Complements the sign/verify unit tests in
//! `src/multiplayer/network/protocol.rs` by proving the *serialized* envelope
//! survives a JSON round-trip and still verifies — i.e. a signed message can be
//! sent over the wire and validated by the receiver. `NetworkMessage` has no
//! `PartialEq`, so structural equality is checked by re-serialization.

use xfchess::multiplayer::network::protocol::{NetworkMessage, SignedNetworkMessage};

fn test_key() -> [u8; 32] {
    let mut seed = [0u8; 32];
    for (i, b) in seed.iter_mut().enumerate() {
        *b = i as u8;
    }
    seed
}

fn move_msg() -> NetworkMessage {
    NetworkMessage::Move {
        game_id: 42,
        turn: 5,
        move_uci: "e2e4".to_string(),
        next_fen: "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1".to_string(),
        nonce: 7,
        timestamp_ms: 1_700_000_000_000,
        agent_id: vec![1, 2, 3],
        seq: 9,
        parent_version: "v0".to_string(),
    }
}

#[test]
fn network_message_json_roundtrip_is_stable() {
    let msg = move_msg();
    let json = serde_json::to_string(&msg).expect("serialize");
    let back: NetworkMessage = serde_json::from_str(&json).expect("deserialize");
    // No PartialEq — assert the re-serialization is identical (structural equality).
    let json2 = serde_json::to_string(&back).expect("re-serialize");
    assert_eq!(json, json2, "NetworkMessage did not survive a JSON round-trip");
    assert_eq!(back.game_id(), 42);
}

#[test]
fn signed_message_survives_serialization_and_still_verifies() {
    let signed = SignedNetworkMessage::sign(move_msg(), &test_key());
    assert!(signed.verify(), "freshly signed message must verify");

    // Send over the wire (serialize) and reconstruct on the other side.
    let json = serde_json::to_string(&signed).expect("serialize signed");
    let received: SignedNetworkMessage = serde_json::from_str(&json).expect("deserialize signed");

    assert!(
        received.verify(),
        "signed message must still verify after a serialization round-trip"
    );
}

#[test]
fn resign_message_signs_serializes_and_verifies() {
    let msg = NetworkMessage::Resign {
        game_id: 99,
        winner: "white".to_string(),
        nonce: 3,
    };
    let signed = SignedNetworkMessage::sign(msg, &test_key());
    let json = serde_json::to_string(&signed).expect("serialize");
    let received: SignedNetworkMessage = serde_json::from_str(&json).expect("deserialize");
    assert!(received.verify());
    assert_eq!(received.msg.game_id(), 99);
}
