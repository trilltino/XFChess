use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

/// Application error type
#[derive(Debug, Error)]
pub enum AppError {
    #[error("Bad request: {0}")]
    BadRequest(String),
    #[error("Internal server error: {0}")]
    Internal(String),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Tournament format not supported: {0}")]
    UnsupportedTournamentFormat(String),
    #[error("WebSocket subscription error: {0}")]
    WebSocketSubscriptionError(String),
    #[error("Configuration error: {0}")]
    ConfigurationError(String),
    #[error("Tournament not found: {0}")]
    TournamentNotFound(u64),
    #[error("Invalid tournament status: {0}")]
    InvalidTournamentStatus(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AppError::UnsupportedTournamentFormat(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::WebSocketSubscriptionError(msg) => (StatusCode::BAD_GATEWAY, msg),
            AppError::ConfigurationError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            AppError::TournamentNotFound(id) => (StatusCode::NOT_FOUND, format!("Tournament {} not found", id)),
            AppError::InvalidTournamentStatus(msg) => (StatusCode::CONFLICT, msg),
        };

        let body = json!({
            "error": message,
        });

        (status, Json(body)).into_response()
    }
}

/// Application result type
pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;
    use axum::response::IntoResponse;

    #[test]
    fn bad_request_status() {
        let err = AppError::BadRequest("missing field".to_string());
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn internal_error_status() {
        let err = AppError::Internal("db down".to_string());
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn not_found_status() {
        let err = AppError::NotFound("user 42".to_string());
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}
