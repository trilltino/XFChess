pub mod api {
    use axum::Router;
    use sqlx::SqlitePool;
    pub fn router(_pool: SqlitePool) -> Router {
        Router::new()
    }
}
pub mod db {
    pub mod schema {
        use sqlx::SqlitePool;
        pub async fn init_db(_pool: &SqlitePool) -> Result<(), sqlx::Error> {
            Ok(())
        }
    }
}
pub mod game {
    pub mod channels {
        #[derive(Debug, Clone)]
        pub struct GameResult {
            pub white_player_id: String,
            pub black_player_id: String,
            pub winner_id: String,
            pub reason: String,
            pub final_fen: String,
        }
    }
    pub fn run_game_server(_db_tx: tokio::sync::mpsc::Sender<channels::GameResult>) {
        println!("Dummy game server running");
    }
}
