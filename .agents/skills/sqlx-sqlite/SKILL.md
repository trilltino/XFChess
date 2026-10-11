---
name: sqlx-sqlite
description: Change XFChess SQLx/SQLite queries, migrations, connection pools, durable game records or native credential metadata. Use for database consistency, contention and recovery.
metadata:
  provenance: docs-derived
---

# SQLx and SQLite

Read the target manifest, migrations and pool setup. The backend and Tauri use
SQLx 0.9 with SQLite; keep those drivers and the existing migration runner.

- Bind values; allowlist identifiers when dynamic SQL is unavoidable. Keep stored
  integers, timestamps, JSON formats and nullability compatible with existing rows.
- Put related writes and idempotency keys in a transaction with database constraints.
  A prior SELECT alone does not prevent concurrent duplicate inserts or payouts.
- SQLite has one writer at a time, including in WAL mode. Keep write transactions
  short, bound busy waits and avoid remote calls while holding a transaction.
- Decide WAL/checkpoint behavior from the deployed storage and contention evidence.
  Do not assume WAL supports a database shared across hosts on network storage.
- Add a new forward migration rather than rewriting an applied migration. Check
  existing data, index coverage, foreign keys and transaction boundaries.
- Use SQLite backup facilities or a coordinated snapshot; copying only the main
  database while writers are active can omit WAL data. Verify restoration in isolation.
- Keep secrets in the existing native keyring or configured secret storage, rather
  than introducing frontend storage or new plaintext database fields.

Use a temporary database for affected query/migration tests. Check reopen/recovery,
constraint rejection and concurrent writers where those are relevant.

Sources: [SQLx](https://docs.rs/sqlx/0.9.0/sqlx/),
[SQLite WAL](https://www.sqlite.org/wal.html),
[transactions](https://www.sqlite.org/lang_transaction.html),
[online backup](https://www.sqlite.org/backup.html).
