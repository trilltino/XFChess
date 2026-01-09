use backend::{api, game};

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::net::SocketAddr;
use std::str::FromStr;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    dotenv::dotenv().ok();

    // Database Connection
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:xfchess.db".to_string());

    let options = SqliteConnectOptions::from_str(&database_url)
        .expect("Invalid database URL")
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await
        .expect("Failed to connect to database");

    // Initialize Schema
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS users (
            id TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
            username TEXT NOT NULL UNIQUE,
            email TEXT NOT NULL UNIQUE,
            password_hash TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );",
    )
    .execute(&pool)
    .await
    .expect("Failed to initialize database schema");

    // Start Axum in a background task
    tokio::spawn(async move {
        let app = api::router(pool);
        let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
        println!("API listening on {}", addr);
        let listener = TcpListener::bind(addr).await.unwrap();
        axum::serve(listener, app).await.unwrap();
    });

    // Start Bevy (blocks main thread)
    println!("Starting Game Server on UDP 5000...");
    game::run_game_server();
}
