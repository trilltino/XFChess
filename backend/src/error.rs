use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

/// Application error type
#[derive(Debug)]
pub enum AppError {
    BadRequest(String),
    Internal(String),
    NotFound(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
        };

        let body = json!({
            "error": message,
        });

        (status, Json(body)).into_response()
    }
}

/// Application result type
pub type AppResult<T> = Result<T, AppError>;
