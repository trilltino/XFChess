# Disk pressure

Database writes, backup failures or resource alerts can indicate a full filesystem.

Check `df -h`, database/WAL sizes, journald and release/build directories.
Inspect `/opt/xfchess/data/backups`, private pre-release snapshots under
`backups/` and releases outside the `current`, `previous` and `ops` links.

Verify successful offsite copies before retiring encrypted backups. Apply the
configured retention and prune only explicitly retired releases/recovery points.
Journald can be bounded with `journalctl --vacuum-size=200M`.

A large WAL needs investigation; never delete it independently of its database.
For a controlled checkpoint, stop the service and run
`sqlite3 <database> 'PRAGMA wal_checkpoint(TRUNCATE);'`, then restart.

Verify free space, database readiness and a successful backup. See
[recovery](../../ops/docs/ROLLBACK_GUIDE.md) and [capacity](../../ops/docs/CAPACITY.md).
