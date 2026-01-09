//! Networking Tests
//!
//! Tests for NetworkMoveEvent handling and message serialization.

use bevy::prelude::*;
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
fn test_game_message_chat() {
    let msg = GameMessage::ChatMessage {
        sender: "Player1".to_string(),
        content: "Hello!".to_string(),
        timestamp: 1234567890,
    };

    match msg {
        GameMessage::ChatMessage {
            sender,
            content,
            timestamp,
        } => {
            assert_eq!(sender, "Player1");
            assert_eq!(content, "Hello!");
            assert_eq!(timestamp, 1234567890);
        }
        _ => panic!("Wrong message type"),
    }
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
