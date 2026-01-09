use crate::auth;
use axum::{
    extract::{Json, State},
    routing::post,
    Router,
};
use base64::prelude::*;
use rand::Rng;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Sqlite};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct AppState {
    // Store lobbies: RoomCode -> LobbyData
    lobbies: Arc<Mutex<std::collections::HashMap<String, Lobby>>>,
    // Database Pool
    pub db: Pool<Sqlite>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Lobby {
    pub id: String,
    pub host_token: Option<String>,
    pub guest_token: Option<String>,
}

#[derive(Serialize)]
pub struct CreateLobbyResponse {
    pub room_code: String,
    pub connect_token: String, // Base64 encoded Lightyear ConnectToken
}

#[derive(Deserialize)]
pub struct JoinLobbyRequest {
    pub room_code: String,
}

#[derive(Serialize)]
pub struct JoinLobbyResponse {
    pub connect_token: String,
    pub player_id: u64, // 1 for host, 2 for guest
}

pub fn router(db: Pool<Sqlite>) -> Router {
    let state = AppState {
        lobbies: Arc::new(Mutex::new(std::collections::HashMap::new())),
        db,
    };

    Router::new()
        .route("/lobby", post(create_lobby))
        .route("/join", post(join_lobby))
        .route("/auth/register", post(auth::register))
        .route("/auth/login", post(auth::login))
        .with_state(state)
}

fn generate_token(client_id: u64) -> String {
    // Must match server config in game.rs
    let server_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 5000);
    let protocol_id = 0;
    // STUB: Real token generation disabled due to Lightyear API mismatch
    let token = format!("DISABLED_TOKEN_FOR_AUTH_TESTING_{}", client_id);
    let token_base64 = base64::engine::general_purpose::STANDARD.encode(token);
    token_base64
}

async fn create_lobby(State(state): State<AppState>) -> Json<CreateLobbyResponse> {
    let room_code = generate_room_code();

    // Host gets ClientId 1
    let connect_token = generate_token(1);

    let lobby = Lobby {
        id: room_code.clone(),
        host_token: Some(connect_token.clone()),
        guest_token: None,
    };

    state
        .lobbies
        .lock()
        .unwrap()
        .insert(room_code.clone(), lobby);

    Json(CreateLobbyResponse {
        room_code,
        connect_token,
    })
}

async fn join_lobby(
    State(state): State<AppState>,
    Json(payload): Json<JoinLobbyRequest>,
) -> Json<JoinLobbyResponse> {
    let mut lobbies = state.lobbies.lock().unwrap();

    if let Some(lobby) = lobbies.get_mut(&payload.room_code) {
        // Guest gets ClientId 2 (in a real app, generate unique IDs)
        let connect_token = generate_token(2);
        lobby.guest_token = Some(connect_token.clone());

        Json(JoinLobbyResponse {
            connect_token,
            player_id: 2,
        })
    } else {
        // In a real app, return 404
        panic!("Lobby not found");
    }
}

fn generate_room_code() -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut rng = rand::thread_rng();
    (0..8)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_room_code_length() {
        let code = generate_room_code();
        assert_eq!(code.len(), 8, "Room code should be 8 characters");
    }

    #[test]
    fn test_generate_room_code_charset() {
        let code = generate_room_code();
        for c in code.chars() {
            assert!(
                c.is_ascii_uppercase() || c.is_ascii_digit(),
                "Room code should only contain uppercase letters and digits"
            );
        }
    }

    #[test]
    fn test_generate_room_code_uniqueness() {
        let code1 = generate_room_code();
        let code2 = generate_room_code();
        // Very unlikely to be equal (1 in 2.8 trillion)
        assert_ne!(code1, code2, "Room codes should be unique");
    }

    #[test]
    fn test_generate_token_returns_base64() {
        let token = generate_token(1);
        // Should be valid base64
        assert!(
            base64::engine::general_purpose::STANDARD
                .decode(&token)
                .is_ok(),
            "Token should be valid base64"
        );
    }

    #[test]
    fn test_lobby_struct_creation() {
        let lobby = Lobby {
            id: "TEST1234".to_string(),
            host_token: Some("token1".to_string()),
            guest_token: None,
        };

        assert_eq!(lobby.id, "TEST1234");
        assert!(lobby.host_token.is_some());
        assert!(lobby.guest_token.is_none());
    }

    #[test]
    fn test_create_lobby_response_serialization() {
        let response = CreateLobbyResponse {
            room_code: "ABCD1234".to_string(),
            connect_token: "dGVzdA==".to_string(),
        };

        let json = serde_json::to_string(&response).expect("Should serialize");
        assert!(json.contains("ABCD1234"));
        assert!(json.contains("dGVzdA=="));
    }

    #[test]
    fn test_join_lobby_request_deserialization() {
        let json = r#"{"room_code": "WXYZ5678"}"#;
        let request: JoinLobbyRequest = serde_json::from_str(json).expect("Should deserialize");
        assert_eq!(request.room_code, "WXYZ5678");
    }

    #[test]
    fn test_join_lobby_response_serialization() {
        let response = JoinLobbyResponse {
            connect_token: "dG9rZW4=".to_string(),
            player_id: 2,
        };

        let json = serde_json::to_string(&response).expect("Should serialize");
        assert!(json.contains("dG9rZW4="));
        assert!(json.contains("2"));
    }
}
