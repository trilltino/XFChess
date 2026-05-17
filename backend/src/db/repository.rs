use sqlx::SqlitePool;
use serde::{Deserialize, Serialize};
use anyhow::Result;

/// Database record for a game
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct GameRecord {
    pub id: String,
    pub player_white: Option<String>,
    pub player_black: Option<String>,
    pub white_username: Option<String>,
    pub black_username: Option<String>,
    pub stake_amount: f64,
    pub fee_lamports: i64,
    pub start_time: i64,
    pub end_time: Option<i64>,
    pub winner: Option<String>,
    pub final_fen: Option<String>,
    pub finalize_sig: Option<String>,
    pub status: String,
    pub archived_at: Option<i64>,
    pub pgn_text: Option<String>,
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

/// Lightweight move record stored by the VPS handler (no SAN/fen_before needed at record time)
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SimpleMoveRecord {
    pub id: Option<i64>,
    pub game_id: String,
    pub move_number: i32,
    pub move_uci: String,
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

    /// Ensures a game row exists (called on first move). Uses INSERT OR IGNORE so it
    /// is safe to call multiple times — the first call wins.
    pub async fn upsert_game(&self, game_id: &str) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            r#"INSERT OR IGNORE INTO games (id, stake_amount, fee_lamports, start_time, status, created_at)
               VALUES (?, 0.0, 0, ?, 'playing', ?)
            "#,
        )
        .bind(game_id)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Finalises the game row: sets wallets, usernames, winner, sig, end_time, status.
    /// Called from the finalize_game VPS handler after on-chain tx succeeds.
    pub async fn complete_game(
        &self,
        game_id: &str,
        player_white: Option<&str>,
        player_black: Option<&str>,
        white_username: Option<&str>,
        black_username: Option<&str>,
        winner: Option<&str>,
        final_fen: Option<&str>,
        finalize_sig: &str,
        stake_amount: f64,
    ) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            r#"INSERT INTO games
               (id, player_white, player_black, white_username, black_username,
                stake_amount, fee_lamports, start_time, end_time,
                winner, final_fen, finalize_sig, status, created_at)
               VALUES (?, ?, ?, ?, ?, ?, 0, ?, ?, ?, ?, ?, 'completed', ?)
               ON CONFLICT(id) DO UPDATE SET
                   player_white   = excluded.player_white,
                   player_black   = excluded.player_black,
                   white_username = excluded.white_username,
                   black_username = excluded.black_username,
                   stake_amount   = excluded.stake_amount,
                   end_time       = excluded.end_time,
                   winner         = excluded.winner,
                   final_fen      = excluded.final_fen,
                   finalize_sig   = excluded.finalize_sig,
                   status         = 'completed'
            "#,
        )
        .bind(game_id)
        .bind(player_white)
        .bind(player_black)
        .bind(white_username)
        .bind(black_username)
        .bind(stake_amount)
        .bind(now) // start_time (only used when no prior row)
        .bind(now) // end_time
        .bind(winner)
        .bind(final_fen)
        .bind(finalize_sig)
        .bind(now) // created_at
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Appends a single move with optional SAN. Called from record_move handler.
    pub async fn add_move_simple(
        &self,
        game_id: &str,
        move_number: i32,
        move_uci: &str,
        move_san: Option<&str>,
        fen_after: Option<&str>,
        player: &str,
    ) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            r#"INSERT OR IGNORE INTO moves (game_id, move_number, move_uci, move_san, fen_after, player, timestamp)
               VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(game_id)
        .bind(move_number)
        .bind(move_uci)
        .bind(move_san)
        .bind(fen_after)
        .bind(player)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Gets the next move number for a game (current max + 1).
    pub async fn get_next_move_number(&self, game_id: &str) -> Result<i64> {
        let result: Option<(i64,)> = sqlx::query_as(
            "SELECT COALESCE(MAX(move_number), 0) FROM moves WHERE game_id = ?"
        )
        .bind(game_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(result.map(|(n,)| n + 1).unwrap_or(1))
    }

    /// Looks up a username from the users_v2 table by wallet pubkey.
    pub async fn get_username(&self, wallet: &str) -> Result<String> {
        let result: (String,) = sqlx::query_as(
            "SELECT username FROM users_v2 WHERE wallet = ? AND deleted_at IS NULL"
        )
        .bind(wallet)
        .fetch_one(&self.pool)
        .await?;
        Ok(result.0)
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

    /// Get all games for a specific player wallet (as white or black)
    pub async fn get_games_by_player(
        &self,
        wallet: &str,
        limit: i32,
    ) -> Result<Vec<GameRecord>> {
        let games = sqlx::query_as::<_, GameRecord>(
            "SELECT * FROM games WHERE player_white = ? OR player_black = ? ORDER BY start_time DESC LIMIT ?"
        )
        .bind(wallet)
        .bind(wallet)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(games)
    }

    /// Get all games for a specific player username (as white or black)
    pub async fn get_games_by_username(
        &self,
        username: &str,
        limit: i32,
    ) -> Result<Vec<GameRecord>> {
        let games = sqlx::query_as::<_, GameRecord>(
            "SELECT * FROM games WHERE white_username = ? OR black_username = ? ORDER BY start_time DESC LIMIT ?"
        )
        .bind(username)
        .bind(username)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(games)
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
 
    /// Gets all completed but not yet archived games
    pub async fn get_unarchived_games(&self, limit: i32) -> Result<Vec<GameRecord>> {
        let games = sqlx::query_as::<_, GameRecord>(
            "SELECT * FROM games WHERE status = 'completed' AND archived_at IS NULL ORDER BY end_time ASC LIMIT ?"
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(games)
    }
 
    /// Store pre-assembled PGN text for a game
    pub async fn set_pgn_text(&self, game_id: &str, pgn: &str) -> Result<()> {
        sqlx::query("UPDATE games SET pgn_text = ? WHERE id = ?")
            .bind(pgn)
            .bind(game_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Retrieve stored PGN text for a game
    pub async fn get_pgn_text(&self, game_id: &str) -> Result<Option<String>> {
        let result: Option<(String,)> = sqlx::query_as(
            "SELECT pgn_text FROM games WHERE id = ?"
        )
        .bind(game_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(result.map(|(pgn,)| pgn))
    }

    /// Marks a game as archived at the given timestamp
    pub async fn mark_as_archived(&self, game_id: &str, timestamp: i64) -> Result<()> {
        sqlx::query("UPDATE games SET archived_at = ? WHERE id = ?")
            .bind(timestamp)
            .bind(game_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Lists all active game sessions
    pub async fn list_active_sessions(&self) -> Result<Vec<serde_json::Value>> {
        let rows: Vec<sqlx::sqlite::SqliteRow> = sqlx::query(
            "SELECT * FROM active_sessions WHERE status = 'active'"
        )
        .fetch_all(&self.pool)
        .await?;
        
        let mut sessions = Vec::new();
        for row in rows {
            use sqlx::Row;
            sessions.push(serde_json::json!({
                "game_id": row.get::<i64, _>("game_id"),
                "white": row.get::<String, _>("player_white"),
                "black": row.get::<String, _>("player_black"),
                "fen": row.get::<String, _>("current_fen"),
                "status": row.get::<String, _>("status"),
                "last_activity": row.get::<i64, _>("last_activity"),
            }));
        }
        Ok(sessions)
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

/// Database record for a dispute
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DisputeRecord {
    pub game_id: i64,
    pub challenger: String,
    pub reason: String,
    pub status: String,
    pub anticheat_score: Option<f64>,
    pub report_path: Option<String>,
    pub decision: Option<String>,
    pub resolution_text: Option<String>,
    pub tx_sig: Option<String>,
    pub notified_at: i64,
    pub analysed_at: Option<i64>,
    pub resolved_at: Option<i64>,
}

/// Repository for dispute database operations
pub struct DisputeRepository {
    pool: SqlitePool,
}

impl DisputeRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn insert(&self, game_id: i64, challenger: &str, reason: &str) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs() as i64;
        sqlx::query(
            "INSERT OR IGNORE INTO disputes (game_id, challenger, reason, notified_at) VALUES (?, ?, ?, ?)"
        )
        .bind(game_id)
        .bind(challenger)
        .bind(reason)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get(&self, game_id: i64) -> Result<Option<DisputeRecord>> {
        let rec = sqlx::query_as::<_, DisputeRecord>(
            "SELECT * FROM disputes WHERE game_id = ?"
        )
        .bind(game_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(rec)
    }

    pub async fn set_resolved(
        &self,
        game_id: i64,
        decision: &str,
        resolution_text: &str,
        tx_sig: &str,
    ) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs() as i64;
        sqlx::query(
            "UPDATE disputes SET status = 'resolved', decision = ?, resolution_text = ?, tx_sig = ?, resolved_at = ? WHERE game_id = ?"
        )
        .bind(decision)
        .bind(resolution_text)
        .bind(tx_sig)
        .bind(now)
        .bind(game_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
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
