use crate::api::AppState;
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use sqlx::Row;

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub user_id: String,
    pub username: String,
}

#[derive(Serialize, Deserialize)]
struct Claims {
    sub: String, // user_id
    exp: usize,
}

pub async fn register(
    State(state): State<AppState>,
    Json(payload): Json<RegisterRequest>,
) -> impl IntoResponse {
    // 1. Hash Password
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(payload.password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Password hashing failed"))?;

    // 2. Insert into DB (using runtime query)
    let result = sqlx::query(
        "INSERT INTO users (username, email, password_hash) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(&payload.username)
    .bind(&payload.email)
    .bind(&password_hash)
    .fetch_one(&state.db)
    .await;

    match result {
        Ok(_) => Ok(StatusCode::CREATED),
        Err(e) => {
            // SQLite unique constraint violation
            if e.to_string().contains("UNIQUE constraint failed") {
                Err((StatusCode::CONFLICT, "Username or Email already exists"))
            } else {
                Err((StatusCode::INTERNAL_SERVER_ERROR, "Database error"))
            }
        }
    }
}

pub async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> impl IntoResponse {
    // 1. Fetch User (using runtime query)
    let user = sqlx::query("SELECT id, username, password_hash FROM users WHERE email = $1")
        .bind(&payload.email)
        .fetch_optional(&state.db)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;

    let user = match user {
        Some(u) => u,
        None => return Err((StatusCode::UNAUTHORIZED, "Invalid credentials")),
    };

    // SQLite stores UUID as TEXT. sqlx reads as String.
    let user_id_str: String = user.get("id");
    let user_id = uuid::Uuid::parse_str(&user_id_str).unwrap_or_default();
    let username: String = user.get("username");
    let password_hash_str: String = user.get("password_hash");

    // 2. Verify Password
    let parsed_hash = PasswordHash::new(&password_hash_str)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Hash parse error"))?;

    if Argon2::default()
        .verify_password(payload.password.as_bytes(), &parsed_hash)
        .is_err()
    {
        return Err((StatusCode::UNAUTHORIZED, "Invalid credentials"));
    }

    // 3. Generate JWT
    let expiration = Utc::now()
        .checked_add_signed(Duration::days(7))
        .expect("valid timestamp")
        .timestamp();

    let claims = Claims {
        sub: user_id.to_string(),
        exp: expiration as usize,
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret("secret".as_ref()), // TODO: Use env var
    )
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Token generation failed"))?;

    Ok(Json(LoginResponse {
        token,
        user_id: user_id.to_string(),
        username,
    }))
}
