use anyhow::Context;
use xfchess_app::multiplayer::network::protocol::{NetworkMessage, SignedNetworkMessage};

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
        signer_pubkey: vec![1, 2, 3],
        seq: 9,
        parent_version: "v0".to_string(),
    }
}

#[test]
fn network_message_json_roundtrip_is_stable() -> anyhow::Result<()> {
    let msg = move_msg();
    let json = serde_json::to_string(&msg).context("serialize")?;
    let back: NetworkMessage = serde_json::from_str(&json).context("deserialize")?;
    // No PartialEq — assert the re-serialization is identical (structural equality).
    let json2 = serde_json::to_string(&back).context("re-serialize")?;
    assert_eq!(
        json, json2,
        "NetworkMessage did not survive a JSON round-trip"
    );
    assert_eq!(back.game_id(), 42);
    Ok(())
}

#[test]
fn signed_message_survives_serialization_and_still_verifies() -> anyhow::Result<()> {
    let signed = SignedNetworkMessage::sign(move_msg(), &test_key())?;
    assert!(signed.verify(), "freshly signed message must verify");

    let json = serde_json::to_string(&signed).context("serialize signed")?;
    let received: SignedNetworkMessage =
        serde_json::from_str(&json).context("deserialize signed")?;

    assert!(
        received.verify(),
        "signed message must still verify after a serialization round-trip"
    );
    Ok(())
}

#[test]
fn resign_message_signs_serializes_and_verifies() -> anyhow::Result<()> {
    let msg = NetworkMessage::Resign {
        game_id: 99,
        winner: "white".to_string(),
        nonce: 3,
    };
    let signed = SignedNetworkMessage::sign(msg, &test_key())?;
    let json = serde_json::to_string(&signed).context("serialize")?;
    let received: SignedNetworkMessage = serde_json::from_str(&json).context("deserialize")?;
    assert!(received.verify());
    assert_eq!(received.msg.game_id(), 99);
    Ok(())
}

#[test]
fn signer_pubkey_still_serializes_under_the_legacy_wire_name() -> anyhow::Result<()> {
    let json = serde_json::to_string(&move_msg()).context("serialize")?;
    assert!(
        json.contains("\"agent_id\""),
        "wire name must stay `agent_id` for compatibility, got: {json}"
    );
    assert!(
        !json.contains("\"signer_pubkey\""),
        "the Rust-side name must not leak onto the wire, got: {json}"
    );

    // A payload written by an old-build peer is byte-identical to the above,
    // so decoding it is the compatibility check in both directions.
    let back: NetworkMessage = serde_json::from_str(&json).context("decode legacy payload")?;
    match back {
        NetworkMessage::Move { signer_pubkey, .. } => {
            assert_eq!(
                signer_pubkey,
                vec![1, 2, 3],
                "value must survive the rename"
            )
        }
        _ => panic!("expected a Move"),
    }
    Ok(())
}
