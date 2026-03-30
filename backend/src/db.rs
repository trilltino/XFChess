// Database module for backend
// Placeholder implementation

pub type DbPool = sqlx::SqlitePool;

pub async fn init_db() -> Result<DbPool, sqlx::Error> {
    DbPool::connect("sqlite://sessions.db?mode=rwc").await
}
