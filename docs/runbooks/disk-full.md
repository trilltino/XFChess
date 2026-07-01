# VPS disk pressure / full

**Symptom:** `disk_space` health check warns/critical; writes failing; SQLite errors;
backup job failing.
**Severity:** S2 → S1 (SQLite can't write = outage).
**Dashboards:** `deploy/monitoring/rules/disk_alerts.yml`; `/health/detailed` disk check.

## Diagnose
1. `ssh $SERVER df -h` — which mount is full?
2. `ssh $SERVER du -sh /opt/xfchess/* /var/log/* 2>/dev/null | sort -h | tail`.
3. Common culprits: local backup copies (`/opt/xfchess/data/backups`), journald logs,
   SQLite WAL growth, old release artifacts.

## Mitigate
1. Prune local backups (offsite copies remain in R2): they auto-prune at
   `BACKUP_RETENTION_DAYS`, but you can delete `*.age` older than N days now.
2. `journalctl --vacuum-size=200M` to trim logs.
3. Checkpoint SQLite WAL if huge: `sqlite3 <db> 'PRAGMA wal_checkpoint(TRUNCATE);'`
   (stop backend first for a clean checkpoint).
4. Remove old deploy artifacts.

## Verify
1. `df -h` shows headroom; `disk_space` check returns ok; backups resume.

## Root cause / follow-up
- Set/verify log rotation + backup retention; add a disk-growth alert threshold if missing;
  consider a larger volume before launch (capacity planning, WS-C).
