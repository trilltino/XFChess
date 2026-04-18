use sqlx::SqlitePool;
use serde::{Deserialize, Serialize};
use anyhow::Result;

/// Database record for a game
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct GameRecord {
    pub id: String,
    pub player_white: String,
    pub player_black: String,
    pub stake_amount: f64,
    pub start_time: i64,
    pub end_time: Option<i64>,
    pub winner: Option<String>,
    pub final_fen: Option<String>,
    pub status: String,
    pub created_at: i64,
}

/// Database record for a move
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct MoveRecord {
    pub id: Option<i64>,
    pub game_id: String,
    pub move_number: i32,
    pub move_uci: String,
    pub move_san: Option<String>,
    pub fen_before: Option<String>,
    pub fen_after: Option<String>,
    pub player: String,
    pub timestamp: i64,
}

/// Repository for game database operations
pub struct GameRepository {
    pool: SqlitePool,
}

impl GameRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Create a new game record
    pub async fn create_game(
        &self,
        game_id: &str,
        player_white: &str,
        player_black: &str,
        stake_amount: f64,
    ) -> Result<GameRecord> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs() as i64;

        let game = sqlx::query_as::<_, GameRecord>(
            r#"
            INSERT INTO games (id, player_white, player_black, stake_amount, start_time, status)
            VALUES (?, ?, ?, ?, ?, 'playing')
            RETURNING *
            "#,
        )
        .bind(game_id)
        .bind(player_white)
        .bind(player_black)
        .bind(stake_amount)
        .bind(now)
        .fetch_one(&self.pool)
        .await?;

        Ok(game)
    }

    /// Get a game by ID
    pub async fn get_game(&self, game_id: &str) -> Result<Option<GameRecord>> {
        let game = sqlx::query_as::<_, GameRecord>(
            "SELECT * FROM games WHERE id = ?"
        )
        .bind(game_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(game)
    }

    /// List all games, optionally with pagination
    pub async fn list_games(&self, limit: Option<i32>, offset: Option<i32>) -> Result<Vec<GameRecord>> {
        let query = match (limit, offset) {
            (Some(limit), Some(offset)) => {
                sqlx::query_as::<_, GameRecord>(
                    "SELECT * FROM games ORDER BY start_time DESC LIMIT ? OFFSET ?"
                )
                .bind(limit)
                .bind(offset)
            }
            (Some(limit), None) => {
                sqlx::query_as::<_, GameRecord>(
                    "SELECT * FROM games ORDER BY start_time DESC LIMIT ?"
                )
                .bind(limit)
            }
            (None, Some(offset)) => {
                sqlx::query_as::<_, GameRecord>(
                    "SELECT * FROM games ORDER BY start_time DESC OFFSET ?"
                )
                .bind(offset)
            }
            (None, None) => {
                sqlx::query_as::<_, GameRecord>(
                    "SELECT * FROM games ORDER BY start_time DESC"
                )
            }
        };

        let games = query.fetch_all(&self.pool).await?;
        Ok(games)
    }

    /// Add a move to a game
    pub async fn add_move(
        &self,
        game_id: &str,
        move_number: i32,
        move_uci: &str,
        move_san: Option<&str>,
        fen_before: Option<&str>,
        fen_after: Option<&str>,
        player: &str,
    ) -> Result<MoveRecord> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs() as i64;

        let move_record = sqlx::query_as::<_, MoveRecord>(
            r#"
            INSERT INTO moves (game_id, move_number, move_uci, move_san, fen_before, fen_after, player, timestamp)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            RETURNING *
            "#,
        )
        .bind(game_id)
        .bind(move_number)
        .bind(move_uci)
        .bind(move_san)
        .bind(fen_before)
        .bind(fen_after)
        .bind(player)
        .bind(now)
        .fetch_one(&self.pool)
        .await?;

        Ok(move_record)
    }

    /// Get all moves for a game
    pub async fn get_moves(&self, game_id: &str) -> Result<Vec<MoveRecord>> {
        let moves = sqlx::query_as::<_, MoveRecord>(
            "SELECT * FROM moves WHERE game_id = ? ORDER BY move_number ASC"
        )
        .bind(game_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(moves)
    }

    /// Update game status and winner
    pub async fn end_game(
        &self,
        game_id: &str,
        winner: Option<&str>,
        final_fen: Option<&str>,
        status: &str,
    ) -> Result<GameRecord> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs() as i64;

        let game = sqlx::query_as::<_, GameRecord>(
            r#"
            UPDATE games 
            SET winner = ?, final_fen = ?, status = ?, end_time = ?
            WHERE id = ?
            RETURNING *
            "#,
        )
        .bind(winner)
        .bind(final_fen)
        .bind(status)
        .bind(now)
        .bind(game_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(game)
    }

    /// Get game statistics
    pub async fn get_stats(&self) -> Result<GameStats> {
        let total_games: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM games")
            .fetch_one(&self.pool)
            .await?;

        let active_games: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM games WHERE status = 'playing'")
            .fetch_one(&self.pool)
            .await?;

        let completed_games: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM games WHERE status = 'completed'")
            .fetch_one(&self.pool)
            .await?;

        let total_moves: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM moves")
            .fetch_one(&self.pool)
            .await?;

        Ok(GameStats {
            total_games,
            active_games,
            completed_games,
            total_moves,
        })
    }

    /// Batch insert moves for performance
    pub async fn batch_add_moves(&self, moves: &[NewMove]) -> Result<()> {
        if moves.is_empty() {
            return Ok(());
        }

        let mut tx = self.pool.begin().await?;

        for new_move in moves {
            sqlx::query(
                r#"
                INSERT INTO moves (game_id, move_number, move_uci, move_san, fen_before, fen_after, player, timestamp)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(&new_move.game_id)
            .bind(new_move.move_number)
            .bind(&new_move.move_uci)
            .bind(&new_move.move_san)
            .bind(&new_move.fen_before)
            .bind(&new_move.fen_after)
            .bind(&new_move.player)
            .bind(new_move.timestamp)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }
}

/// Statistics about games
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameStats {
    pub total_games: i64,
    pub active_games: i64,
    pub completed_games: i64,
    pub total_moves: i64,
}

/// New move for batch insertion
#[derive(Debug, Clone)]
pub struct NewMove {
    pub game_id: String,
    pub move_number: i32,
    pub move_uci: String,
    pub move_san: Option<String>,
    pub fen_before: Option<String>,
    pub fen_after: Option<String>,
    pub player: String,
    pub timestamp: i64,
}
