pub mod api {
    use axum::Router;
    use sqlx::SqlitePool;
    pub fn router(_pool: SqlitePool) -> Router {
        Router::new()
    }
}
pub mod db;
