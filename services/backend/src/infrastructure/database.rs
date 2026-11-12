use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    SqlitePool,
};
use std::str::FromStr;
use tracing::info;

#[derive(Clone)]
pub struct DatabasePools {
    pub session_pool: SqlitePool,
    pub vault_pool: SqlitePool,
}

async fn make_pool(url: &str) -> Result<SqlitePool, sqlx::Error> {
    let options = SqliteConnectOptions::from_str(url)?;
    // `mode=rwc` creates the file but not a missing parent directory.
    if let Some(dir) = options.get_filename().parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir)?;
        }
    }
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
        .connect_with(options)
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
    "015_social_invites",
    "016_wallet_social",
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
    rename_legacy_accounts(pool).await?;
    run_scripts(pool, SESSION_SCRIPTS).await
}

async fn table_exists(
    conn: &mut sqlx::SqliteConnection,
    name: &str,
) -> Result<bool, sqlx::Error> {
    let found: Option<i64> =
        sqlx::query_scalar("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?")
            .bind(name)
            .fetch_optional(&mut *conn)
            .await?;
    Ok(found.is_some())
}

/// Databases made before the accounts table was named `users` hold the wallet accounts in
/// `users_v2` and an unused pre-wallet login table called `users`. Move the old one aside as
/// `legacy_users` (its rows are kept, nothing reads it) and give the accounts their final name.
/// A database that is already current, or brand new, is left alone.
async fn rename_legacy_accounts(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    let has_v2 = table_exists(&mut tx, "users_v2").await?;
    let has_users = table_exists(&mut tx, "users").await?;
    // The pre-wallet table has no `wallet` column.
    let users_is_legacy = has_users && {
        let wallet_columns: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('users') WHERE name = 'wallet'",
        )
        .fetch_one(&mut *tx)
        .await?;
        wallet_columns == 0
    };
    if users_is_legacy {
        if table_exists(&mut tx, "legacy_users").await? {
            // Never overwrite an earlier copy.
            return Err(sqlx::Error::Protocol(
                "both `users` (pre-wallet) and `legacy_users` exist; resolve by hand".into(),
            ));
        }
        sqlx::query("ALTER TABLE users RENAME TO legacy_users")
            .execute(&mut *tx)
            .await?;
    }
    if has_v2 {
        sqlx::query("ALTER TABLE users_v2 RENAME TO users")
            .execute(&mut *tx)
            .await?;
        // 001_accounts.sql recreates the index under its final name.
        sqlx::query("DROP INDEX IF EXISTS idx_users_v2_username")
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await
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

    #[tokio::test]
    async fn users_v2_becomes_users_and_the_pre_wallet_table_is_kept_aside() -> anyhow::Result<()> {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .context("test setup failed")?;
        sqlx::raw_sql(
            "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, password TEXT);
             INSERT INTO users (name, password) VALUES ('old', 'hash');
             CREATE TABLE users_v2 (wallet TEXT PRIMARY KEY, username TEXT NOT NULL, email TEXT, \
             kyc_status TEXT NOT NULL DEFAULT 'none', created_at INTEGER NOT NULL DEFAULT 0, \
             deleted_at INTEGER);
             INSERT INTO users_v2 (wallet, username) VALUES ('w1', 'Tino');
             CREATE UNIQUE INDEX idx_users_v2_username ON users_v2 (LOWER(username));",
        )
        .execute(&pool)
        .await
        .context("test setup failed")?;

        // The second run proves the upgrade does nothing once it has happened.
        for _ in 0..2 {
            migrate_session_db(&pool)
                .await
                .context("migration failed")?;
        }

        let accounts: Vec<(String, String)> = sqlx::query_as("SELECT wallet, username FROM users")
            .fetch_all(&pool)
            .await
            .context("accounts table")?;
        assert_eq!(accounts, [("w1".to_string(), "Tino".to_string())]);
        let legacy: Vec<(String,)> = sqlx::query_as("SELECT name FROM legacy_users")
            .fetch_all(&pool)
            .await
            .context("legacy table")?;
        assert_eq!(legacy, [("old".to_string(),)]);
        let leftover = sqlx::query_scalar::<_, String>(
            "SELECT name FROM sqlite_master WHERE name IN ('users_v2', 'idx_users_v2_username')",
        )
        .fetch_optional(&pool)
        .await
        .context("leftovers")?;
        assert_eq!(leftover, None, "nothing named v2 survives");
        Ok(())
    }
}
