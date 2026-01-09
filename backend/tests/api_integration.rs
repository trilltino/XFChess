//! Backend API Integration Tests
//!
//! Tests for the Axum HTTP endpoints using Router::oneshot pattern.

use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use backend::api;
use serde_json::{json, Value};
use sqlx::sqlite::SqlitePoolOptions;
use tower::ServiceExt;

/// Helper to create a test database pool
async fn test_db() -> sqlx::Pool<sqlx::Sqlite> {
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect(":memory:")
        .await
        .expect("Failed to create test database")
}

/// Helper to create test router
async fn test_router() -> Router {
    let db = test_db().await;
    api::router(db)
}

#[tokio::test]
async fn test_create_lobby_returns_room_code() {
    let app = test_router().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/lobby")
                .header("content-type", "application/json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();

    // Should have room_code and connect_token
    assert!(body.get("room_code").is_some());
    assert!(body.get("connect_token").is_some());

    // Room code should be 8 characters
    let room_code = body["room_code"].as_str().unwrap();
    assert_eq!(room_code.len(), 8);
}

#[tokio::test]
async fn test_room_code_is_alphanumeric() {
    let app = test_router().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/lobby")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();

    let room_code = body["room_code"].as_str().unwrap();
    assert!(
        room_code.chars().all(|c| c.is_ascii_alphanumeric()),
        "Room code should be alphanumeric"
    );
}

#[tokio::test]
async fn test_join_existing_lobby() {
    let app = test_router().await;

    // First create a lobby
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/lobby")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let create_body = axum::body::to_bytes(create_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let create_json: Value = serde_json::from_slice(&create_body).unwrap();
    let room_code = create_json["room_code"].as_str().unwrap();

    // Now join the lobby
    let join_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/join")
                .header("content-type", "application/json")
                .body(Body::from(json!({"room_code": room_code}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(join_response.status(), StatusCode::OK);

    let join_body = axum::body::to_bytes(join_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let join_json: Value = serde_json::from_slice(&join_body).unwrap();

    assert!(join_json.get("connect_token").is_some());
    assert_eq!(join_json["player_id"].as_u64(), Some(2));
}

#[tokio::test]
async fn test_connect_token_is_base64() {
    let app = test_router().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/lobby")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();

    let token = body["connect_token"].as_str().unwrap();

    // Token should be valid base64
    assert!(
        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, token).is_ok(),
        "Connect token should be valid base64"
    );
}
