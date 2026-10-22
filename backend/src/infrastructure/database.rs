use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use tracing::info;

#[derive(Clone)]
pub struct DatabasePools {
    pub session_pool: SqlitePool,
    pub vault_pool: SqlitePool,
}

async fn make_pool(url: &str) -> Result<SqlitePool, sqlx::Error> {
    let pool = SqlitePoolOptions::new()
        .max_connections(16)
        .min_connections(2)
        .acquire_timeout(std::time::Duration::from_secs(10))
        .idle_timeout(std::time::Duration::from_secs(300))
        .after_connect(|conn, _meta| {
            Box::pin(async move {
                // WAL mode: concurrent reads + single writer, no contention
                sqlx::query("PRAGMA journal_mode = WAL")
                    .execute(&mut *conn)
                    .await?;
                // NORMAL sync is safe with WAL (only risks losing the last tx on OS crash)
                sqlx::query("PRAGMA synchronous = NORMAL")
                    .execute(&mut *conn)
                    .await?;
                // Enforce FK constraints
                sqlx::query("PRAGMA foreign_keys = ON")
                    .execute(&mut *conn)
                    .await?;
                // Wait up to 5s on a locked write instead of returning SQLITE_BUSY
                sqlx::query("PRAGMA busy_timeout = 5000")
                    .execute(&mut *conn)
                    .await?;
                Ok(())
            })
        })
        .connect(url)
        .await?;
    Ok(pool)
}

pub async fn initialize_pools(
    session_db_url: &str,
    vault_db_url: &str,
) -> Result<DatabasePools, sqlx::Error> {
    let session_pool = make_pool(session_db_url).await?;
    info!("[Database] Session pool connected (WAL)");

    let vault_pool = make_pool(vault_db_url).await?;
    info!("[Database] Vault pool connected (WAL)");

    Ok(DatabasePools {
        session_pool,
        vault_pool,
    })
}

/// A schema script from `migrations/`. Every script runs on every start, so each
/// must be idempotent (see `MIGRATIONS.md`).
struct Script {
    name: &'static str,
    sql: &'static str,
}

macro_rules! scripts {
    ($dir:literal: $($name:literal),+ $(,)?) => {
        &[$(Script {
            name: concat!($dir, "/", $name),
            sql: include_str!(concat!("../../migrations/", $dir, "/", $name, ".sql")),
        }),+]
    };
}

/// `sessions.db`: everything but identity data.
const SESSION_SCRIPTS: &[Script] = scripts!("session":
    "001_accounts",
    "002_signing_sessions",
    "003_game_history",
    "004_live_games",
    "005_matchmaking",
    "006_anticheat",
    "007_moderation",
    "008_compliance",
    "009_lichess",
    "010_puzzles",
    "011_friends",
    "012_job_queue",
    "013_admin",
    "014_offline_tournaments",
);

/// `vault.db`: identity and KYC data, kept apart from game data.
const VAULT_SCRIPTS: &[Script] = scripts!("vault":
    "001_identity_vault",
    "002_kyc_and_erasure",
);

pub async fn run_migrations(pools: &DatabasePools) -> Result<(), sqlx::Error> {
    migrate_session_db(&pools.session_pool).await?;
    migrate_vault_db(&pools.vault_pool).await?;
    info!("[Database] All migrations completed successfully");
    Ok(())
}

/// Brings a session database's schema up to date.
pub async fn migrate_session_db(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    run_scripts(pool, SESSION_SCRIPTS).await
}

/// Brings a vault database's schema up to date.
pub async fn migrate_vault_db(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    run_scripts(pool, VAULT_SCRIPTS).await
}

async fn run_scripts(pool: &SqlitePool, scripts: &[Script]) -> Result<(), sqlx::Error> {
    for script in scripts {
        for statement in statements(script.sql) {
            // SAFETY: statements come from the scripts compiled into this module,
            // never from input.
            let Err(e) = sqlx::query(sqlx::AssertSqlSafe(statement.as_str()))
                .execute(pool)
                .await
            else {
                continue;
            };
            // An upgrade ALTER on a column the CREATE already made, or an object
            // created by an earlier start.
            let message = e.to_string().to_lowercase();
            if message.contains("duplicate column") || message.contains("already exists") {
                continue;
            }
            tracing::error!("Migration {} failed on `{statement}`: {e}", script.name);
            return Err(e);
        }
    }
    Ok(())
}

/// The script's statements with `--` comments removed, so a `;` inside a comment
/// does not split one.
fn statements(script: &str) -> Vec<String> {
    let code = script
        .lines()
        .map(|line| line.split_once("--").map_or(line, |(code, _)| code))
        .collect::<Vec<_>>()
        .join(" ");
    code.split(';')
        .map(str::trim)
        .filter(|statement| !statement.is_empty())
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Context as _;

    fn listed(dir: &str, scripts: &[Script]) -> Vec<String> {
        let mut names: Vec<String> = scripts
            .iter()
            .map(|s| s.name.trim_start_matches(&format!("{dir}/")).to_string())
            .collect();
        names.sort();
        names
    }

    fn on_disk(dir: &str) -> anyhow::Result<Vec<String>> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("migrations")
            .join(dir);
        let mut names = Vec::new();
        for entry in std::fs::read_dir(path)? {
            let path = entry?.path();
            if path.extension().is_some_and(|extension| extension == "sql") {
                names.push(
                    path.file_stem()
                        .context("migration filename")?
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
        names.sort();
        Ok(names)
    }

    #[test]
    fn every_script_on_disk_is_run() -> anyhow::Result<()> {
        assert_eq!(listed("session", SESSION_SCRIPTS), on_disk("session")?);
        assert_eq!(listed("vault", VAULT_SCRIPTS), on_disk("vault")?);
        let top_level = std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations"))
            .context("test setup failed")?
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .filter(|entry| entry.path().is_file())
            .count();
        assert_eq!(
            top_level, 0,
            "only the session/ and vault/ folders live in migrations/"
        );
        Ok(())
    }

    #[test]
    fn comments_never_split_or_leak_into_statements() {
        let sql = "-- a; b\nCREATE TABLE t (a TEXT); -- trailing; note\n\nALTER TABLE t ADD COLUMN b TEXT;";
        assert_eq!(
            statements(sql),
            vec!["CREATE TABLE t (a TEXT)", "ALTER TABLE t ADD COLUMN b TEXT"]
        );
    }

    #[tokio::test]
    async fn scripts_can_run_on_every_start() -> anyhow::Result<()> {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .context("test setup failed")?;
        migrate_session_db(&pool)
            .await
            .context("test setup failed")?;
        migrate_session_db(&pool)
            .await
            .context("test setup failed")?;
        let vault = SqlitePool::connect("sqlite::memory:")
            .await
            .context("test setup failed")?;
        migrate_vault_db(&vault)
            .await
            .context("test setup failed")?;
        migrate_vault_db(&vault)
            .await
            .context("test setup failed")?;
        Ok(())
    }

    #[tokio::test]
    async fn an_older_database_gets_the_newer_columns() -> anyhow::Result<()> {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .context("test setup failed")?;
        sqlx::raw_sql(
            "CREATE TABLE sessions (game_id INTEGER PRIMARY KEY, keypair BLOB NOT NULL, \
             wallet TEXT NOT NULL, active INTEGER NOT NULL DEFAULT 0);
             CREATE TABLE game_event_log (game_id TEXT NOT NULL, seq INTEGER NOT NULL, \
             kind TEXT NOT NULL, version_hash TEXT NOT NULL, parent_version TEXT NOT NULL, \
             payload_json TEXT NOT NULL, created_at INTEGER NOT NULL, PRIMARY KEY (game_id, seq));
             CREATE INDEX idx_game_event_log_game ON game_event_log(game_id, seq);",
        )
        .execute(&pool)
        .await
        .context("test setup failed")?;
        migrate_session_db(&pool)
            .await
            .context("test setup failed")?;
        let columns: Vec<String> =
            sqlx::query_scalar("SELECT name FROM pragma_table_info('sessions')")
                .fetch_all(&pool)
                .await
                .context("test setup failed")?;
        assert!(columns.ends_with(&["move_count".into(), "is_global".into(), "created_at".into()]));
        let retired: Option<String> = sqlx::query_scalar(
            "SELECT name FROM sqlite_master WHERE name = 'idx_game_event_log_game'",
        )
        .fetch_optional(&pool)
        .await
        .context("test setup failed")?;
        assert_eq!(retired, None, "redundant indexes are dropped");
        let columns: Vec<String> = sqlx::query_scalar(
            "SELECT name FROM pragma_index_info('idx_game_event_log_version') ORDER BY seqno",
        )
        .fetch_all(&pool)
        .await
        .context("test setup failed")?;
        assert_eq!(columns, ["game_id", "version_hash"]);
        Ok(())
    }
}
