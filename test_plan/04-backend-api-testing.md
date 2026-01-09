# Backend API Testing Guide

This guide covers testing the Axum backend server using patterns from `reference/axum/examples/testing/`.

## Core Concept: Router as Service

Axum routers implement `tower::Service`, allowing direct testing without HTTP server:

```rust
use axum::{body::Body, http::{Request, StatusCode}, Router};
use tower::ServiceExt;

async fn test_endpoint() {
    let app: Router = create_app();
    
    let response = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
}
```

## Backend Structure

```
backend/
├── src/
│   ├── main.rs      # Server entry point
│   ├── api.rs       # Route handlers
│   ├── db.rs        # Database operations
│   └── game.rs      # Game server logic
└── tests/
    └── room_flow.rs # Integration tests
```

## Unit Testing Routes

### Basic Route Test

```rust
// backend/src/api.rs
use axum::{routing::get, Router, Json};

pub fn create_router() -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/api/rooms", get(list_rooms))
}

async fn health_check() -> &'static str {
    "OK"
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_health_check() {
        let app = create_router();
        
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap()
            )
            .await
            .unwrap();
        
        assert_eq!(response.status(), StatusCode::OK);
        
        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&body[..], b"OK");
    }
}
```

### Testing JSON Endpoints

```rust
use serde_json::{json, Value};

#[tokio::test]
async fn test_json_response() {
    let app = create_router();
    
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/rooms")
                .body(Body::empty())
                .unwrap()
        )
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    
    assert!(json["rooms"].is_array());
}
```

### Testing POST with JSON Body

```rust
#[tokio::test]
async fn test_create_room() {
    let app = create_router();
    
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/rooms")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "name": "Test Room",
                        "max_players": 2
                    })).unwrap()
                ))
                .unwrap()
        )
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::CREATED);
}
```

### Testing Authentication

```rust
#[tokio::test]
async fn test_protected_route_without_auth() {
    let app = create_router();
    
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/protected")
                .body(Body::empty())
                .unwrap()
        )
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_protected_route_with_auth() {
    let app = create_router();
    
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/protected")
                .header("Authorization", "Bearer valid_token")
                .body(Body::empty())
                .unwrap()
        )
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
}
```

## Testing with State

```rust
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
struct AppState {
    db: Arc<RwLock<Database>>,
}

#[tokio::test]
async fn test_with_mock_state() {
    let mock_db = Database::in_memory();
    let state = AppState {
        db: Arc::new(RwLock::new(mock_db)),
    };
    
    let app = create_router().with_state(state);
    
    // Run tests with mock database
}
```

## Testing Multiple Requests

```rust
use tower::Service;

#[tokio::test]
async fn test_multiple_requests() {
    let mut app = create_router().into_service();
    
    // First request
    let req1 = Request::builder().uri("/").body(Body::empty()).unwrap();
    let res1 = ServiceExt::<Request<Body>>::ready(&mut app)
        .await.unwrap()
        .call(req1)
        .await.unwrap();
    assert_eq!(res1.status(), StatusCode::OK);
    
    // Second request
    let req2 = Request::builder().uri("/health").body(Body::empty()).unwrap();
    let res2 = ServiceExt::<Request<Body>>::ready(&mut app)
        .await.unwrap()
        .call(req2)
        .await.unwrap();
    assert_eq!(res2.status(), StatusCode::OK);
}
```

## Running Backend Tests

```bash
# Run all backend tests
cargo test -p backend

# Run specific test
cargo test -p backend test_health_check

# Run with database (integration)
DATABASE_URL=sqlite::memory: cargo test -p backend
```
