//! Protocol message tests for the `shared` crate.
//!
//! Construction/variant tests for `GameMessage` and `LobbyMessage`. (Moved here
//! from the game crate's `tests/`, where it could not compile — these types live
//! in `shared`, not `xfchess`.)

use shared::protocol::{GameMessage, LobbyMessage};

/// Test basic message creation
#[test]
fn test_game_message_submit_move_creation() {
    let msg = GameMessage::SubmitMove {
        from: (4, 1),
        to: (4, 3),
    };
    match msg {
        GameMessage::SubmitMove { from, to } => {
            assert_eq!(from, (4, 1));
            assert_eq!(to, (4, 3));
        }
        _ => panic!("Wrong message type"),
    }
}

#[test]
fn test_game_message_move_made_creation() {
    let msg = GameMessage::MoveMade {
        from: (4, 6),
        to: (4, 4),
    };
    match msg {
        GameMessage::MoveMade { from, to } => {
            assert_eq!(from, (4, 6));
            assert_eq!(to, (4, 4));
        }
        _ => panic!("Wrong message type"),
    }
}

#[test]
fn test_lobby_message_create_room() {
    let msg = LobbyMessage::CreateRoom;
    assert_eq!(msg, LobbyMessage::CreateRoom);
}

#[test]
fn test_lobby_message_join_room() {
    let msg = LobbyMessage::JoinRoom {
        code: "ABCD1234".to_string(),
    };

    match msg {
        LobbyMessage::JoinRoom { code } => {
            assert_eq!(code, "ABCD1234");
        }
        _ => panic!("Wrong message type"),
    }
}

#[test]
fn test_lobby_message_set_ready() {
    let msg_ready = LobbyMessage::SetReady { ready: true };
    let msg_not_ready = LobbyMessage::SetReady { ready: false };

    assert_eq!(msg_ready, LobbyMessage::SetReady { ready: true });
    assert_eq!(msg_not_ready, LobbyMessage::SetReady { ready: false });
    assert_ne!(msg_ready, msg_not_ready);
}

#[test]
fn test_game_message_resign() {
    let msg = GameMessage::Resign;
    assert_eq!(msg, GameMessage::Resign);
}

#[test]
fn test_game_message_game_end() {
    let msg = GameMessage::GameEnd {
        winner: Some(1),
        reason: "Checkmate".to_string(),
    };

    match msg {
        GameMessage::GameEnd { winner, reason } => {
            assert_eq!(winner, Some(1));
            assert_eq!(reason, "Checkmate");
        }
        _ => panic!("Wrong message type"),
    }
}

#[test]
fn test_game_end_draw() {
    let msg = GameMessage::GameEnd {
        winner: None,
        reason: "Stalemate".to_string(),
    };

    match msg {
        GameMessage::GameEnd { winner, reason } => {
            assert!(winner.is_none());
            assert_eq!(reason, "Stalemate");
        }
        _ => panic!("Wrong message type"),
    }
}

#[test]
fn test_lobby_message_game_starting() {
    let msg_white = LobbyMessage::GameStarting { your_color: true };
    let msg_black = LobbyMessage::GameStarting { your_color: false };

    match msg_white {
        LobbyMessage::GameStarting { your_color } => assert!(your_color),
        _ => panic!("Wrong type"),
    }

    match msg_black {
        LobbyMessage::GameStarting { your_color } => assert!(!your_color),
        _ => panic!("Wrong type"),
    }
}

// ── Wire round-trips ─────────────────────────────────────────────────────────
// These exercise the actual serialization path (bincode, the P2P wire format),
// not just construction. A field reorder / variant change that breaks the wire
// format is caught here.

fn roundtrip_game(msg: &GameMessage) {
    let bytes = bincode::serialize(msg).expect("serialize");
    let back: GameMessage = bincode::deserialize(&bytes).expect("deserialize");
    assert_eq!(*msg, back, "GameMessage did not survive a bincode round-trip");
}

fn roundtrip_lobby(msg: &LobbyMessage) {
    let bytes = bincode::serialize(msg).expect("serialize");
    let back: LobbyMessage = bincode::deserialize(&bytes).expect("deserialize");
    assert_eq!(*msg, back, "LobbyMessage did not survive a bincode round-trip");
}

#[test]
fn game_messages_roundtrip() {
    roundtrip_game(&GameMessage::SubmitMove { from: (4, 1), to: (4, 3) });
    roundtrip_game(&GameMessage::MoveMade { from: (4, 6), to: (4, 4) });
    roundtrip_game(&GameMessage::Resign);
    roundtrip_game(&GameMessage::GameEnd { winner: Some(1), reason: "Checkmate".into() });
    roundtrip_game(&GameMessage::GameEnd { winner: None, reason: "Stalemate".into() });
}

#[test]
fn lobby_messages_roundtrip() {
    roundtrip_lobby(&LobbyMessage::CreateRoom);
    roundtrip_lobby(&LobbyMessage::JoinRoom { code: "ABCD1234".into() });
    roundtrip_lobby(&LobbyMessage::SetReady { ready: true });
    roundtrip_lobby(&LobbyMessage::SetReady { ready: false });
    roundtrip_lobby(&LobbyMessage::GameStarting { your_color: true });
}

#[test]
fn distinct_messages_serialize_differently() {
    // A guard against an accidental shared encoding that would let one message
    // type be misread as another on the wire.
    let a = bincode::serialize(&GameMessage::SubmitMove { from: (4, 1), to: (4, 3) }).unwrap();
    let b = bincode::serialize(&GameMessage::MoveMade { from: (4, 1), to: (4, 3) }).unwrap();
    assert_ne!(a, b, "distinct variants must encode distinctly");
}
