use bevy::prelude::*;
use lightyear::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Channel1;

/// Lobby-related messages for room management
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum LobbyMessage {
    // Client → Server
    CreateRoom,
    JoinRoom { code: String },
    SetReady { ready: bool },
    StartGame, // Host only

    // Server → Client
    RoomCreated { code: String },
    JoinedRoom { code: String, is_host: bool }, // is_host: true=White, false=Black
    PlayerJoined { player_id: u64 },
    PlayerLeft { player_id: u64 },
    PlayerReady { player_id: u64, ready: bool },
    GameStarting { your_color: bool }, // true=White, false=Black
    Error { message: String },
}

/// In-game messages
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum GameMessage {
    // Client → Server
    SubmitMove {
        from: (u8, u8),
        to: (u8, u8),
    },
    Resign,

    // Bidirectional Chat
    ChatMessage {
        sender: String,
        content: String,
        timestamp: u64,
    },

    // Server → Client (Broadcast)
    MoveMade {
        from: (u8, u8),
        to: (u8, u8),
    },

    // Server → Client
    GameStateUpdate {
        board: Vec<i8>,
        turn: i64,
        valid_move: bool,
    },
    GameEnd {
        winner: Option<i64>,
        reason: String,
    },
    CrdtOperation(crate::crdt::MessageOperation),
}

pub struct ProtocolPlugin;

impl Plugin for ProtocolPlugin {
    fn build(&self, app: &mut App) {
        app.add_channel::<Channel1>(ChannelSettings {
            mode: ChannelMode::OrderedReliable(ReliableSettings::default()),
            ..default()
        });

        app.register_message::<LobbyMessage>();
        app.register_message::<GameMessage>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lobby_message_join_room_serialization() {
        let msg = LobbyMessage::JoinRoom {
            code: "ABCD".to_string(),
        };
        let bytes = bincode::serialize(&msg).expect("Should serialize");
        let decoded: LobbyMessage = bincode::deserialize(&bytes).expect("Should deserialize");

        match decoded {
            LobbyMessage::JoinRoom { code } => assert_eq!(code, "ABCD"),
            _ => panic!("Wrong message type after deserialization"),
        }
    }

    #[test]
    fn test_lobby_message_create_room() {
        let msg = LobbyMessage::CreateRoom;
        let bytes = bincode::serialize(&msg).expect("Should serialize");
        let decoded: LobbyMessage = bincode::deserialize(&bytes).expect("Should deserialize");
        assert_eq!(decoded, LobbyMessage::CreateRoom);
    }

    #[test]
    fn test_lobby_message_set_ready() {
        let msg = LobbyMessage::SetReady { ready: true };
        let bytes = bincode::serialize(&msg).expect("Should serialize");
        let decoded: LobbyMessage = bincode::deserialize(&bytes).expect("Should deserialize");
        assert_eq!(decoded, LobbyMessage::SetReady { ready: true });
    }

    #[test]
    fn test_lobby_message_game_starting() {
        let msg = LobbyMessage::GameStarting { your_color: true };
        let bytes = bincode::serialize(&msg).expect("Should serialize");
        let decoded: LobbyMessage = bincode::deserialize(&bytes).expect("Should deserialize");

        match decoded {
            LobbyMessage::GameStarting { your_color } => assert!(your_color),
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_game_message_submit_move() {
        let msg = GameMessage::SubmitMove {
            from: (4, 1),
            to: (4, 3),
        };
        let bytes = bincode::serialize(&msg).expect("Should serialize");
        let decoded: GameMessage = bincode::deserialize(&bytes).expect("Should deserialize");

        match decoded {
            GameMessage::SubmitMove { from, to } => {
                assert_eq!(from, (4, 1));
                assert_eq!(to, (4, 3));
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_game_message_move_made() {
        let msg = GameMessage::MoveMade {
            from: (4, 1),
            to: (4, 3),
        };
        let bytes = bincode::serialize(&msg).expect("Should serialize");
        let decoded: GameMessage = bincode::deserialize(&bytes).expect("Should deserialize");
        assert_eq!(
            decoded,
            GameMessage::MoveMade {
                from: (4, 1),
                to: (4, 3)
            }
        );
    }

    #[test]
    fn test_game_message_resign() {
        let msg = GameMessage::Resign;
        let bytes = bincode::serialize(&msg).expect("Should serialize");
        let decoded: GameMessage = bincode::deserialize(&bytes).expect("Should deserialize");
        assert_eq!(decoded, GameMessage::Resign);
    }

    #[test]
    fn test_game_message_chat() {
        let msg = GameMessage::ChatMessage {
            sender: "Player1".to_string(),
            content: "Hello!".to_string(),
            timestamp: 1234567890,
        };
        let bytes = bincode::serialize(&msg).expect("Should serialize");
        let decoded: GameMessage = bincode::deserialize(&bytes).expect("Should deserialize");

        match decoded {
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
    fn test_game_message_game_end() {
        let msg = GameMessage::GameEnd {
            winner: Some(1),
            reason: "Checkmate".to_string(),
        };
        let bytes = bincode::serialize(&msg).expect("Should serialize");
        let decoded: GameMessage = bincode::deserialize(&bytes).expect("Should deserialize");

        match decoded {
            GameMessage::GameEnd { winner, reason } => {
                assert_eq!(winner, Some(1));
                assert_eq!(reason, "Checkmate");
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_lobby_message_equality() {
        let msg1 = LobbyMessage::JoinRoom {
            code: "TEST".to_string(),
        };
        let msg2 = LobbyMessage::JoinRoom {
            code: "TEST".to_string(),
        };
        let msg3 = LobbyMessage::JoinRoom {
            code: "OTHER".to_string(),
        };

        assert_eq!(msg1, msg2);
        assert_ne!(msg1, msg3);
    }
}
