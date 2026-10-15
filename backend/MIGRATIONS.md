# Backend migrations

The SQLite schema in [migrations/](migrations/), applied at startup by
[`infrastructure/database.rs`](src/infrastructure/database.rs) (`run_migrations`).

| Folder | Database | Contents |
|--------|----------|----------|
| [session/](migrations/session/) | `sessions.db` (`SESSION_DB_URL`) | Everything except identity data |
| [vault/](migrations/vault/) | `vault.db` (`VAULT_DB_URL`) | Encrypted identities, KYC records, GDPR trail |

Each file holds the tables for one area, named after it (`003_game_history.sql`,
`010_puzzles.sql`, ...), and each `CREATE TABLE` shows the table as it is today.

## How it runs

There is no version table. **Every script runs on every start**, in the order
listed in `database.rs`, so every statement must be safe to repeat:

- `CREATE TABLE IF NOT EXISTS` / `CREATE INDEX IF NOT EXISTS`.
- A column added after a table first shipped goes **both** into its `CREATE TABLE`
  (for new databases) **and** into an `ALTER TABLE ... ADD COLUMN` right after it
  (for existing ones). The runner ignores the ALTER's "duplicate column" error.
- An index that is no longer needed is removed with `DROP INDEX IF EXISTS`.
- Never put a `DROP TABLE`, `DELETE` or `UPDATE` here: it would rerun on every start.
  Data clean-ups are one-off scripts run by hand.

`--` comments are stripped before statements are split on `;`, so a comment may
contain a semicolon; a string literal may not contain `--` or `;`.

## Adding to the schema

- **New column**: edit the area's file: add the column to the `CREATE TABLE`
  and an `ALTER TABLE ... ADD COLUMN` after it.
- **New table in an existing area**: add it to that area's file.
- **New area**: add `NNN_area.sql` to `session/` or `vault/` and list it in
  `SESSION_SCRIPTS` / `VAULT_SCRIPTS`. A unit test fails if a file is not listed.

Tests that need these tables call `migrate_session_db` / `migrate_vault_db` on an
in-memory pool instead of copying DDL.

## Not here

These stores still create their own tables on construction:
`TournamentStore` (`tournaments`, `tournament_transactions`,
`tournament_registration_transactions`), `MoneyActionStore` (`money_actions`) and
`tournament_operations` (`tournament_operations`, `archived_tournaments`).

## Left behind in databases created before 2026-10-12

The old scripts created some tables nothing uses. They are not dropped
automatically. Remove them by hand once backed up if you want:

- `sessions.db`: `users` (pre-wallet login, copied into `users_v2`),
  `external_elo_sync_log`, and empty copies of the vault tables `vault_users`,
  `audit_log`, `kyc_records`, `deletion_requests`.
- `vault.db`: empty `sessions` and `users`.
