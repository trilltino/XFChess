# Rollback and recovery

From the repository terminal:

```powershell
powershell -File ops\scripts\rollback.ps1 -Server 178.104.55.19
# Or select an existing release explicitly:
powershell -File ops\scripts\rollback.ps1 -Server 178.104.55.19 -Release <release-id>
```

Rollback switches the backend, website and saved nginx/systemd configuration
together. It uses the same identity and SSH checks as deployment and exits with
an error if readiness or the expected commit cannot be verified. A failed rollback
attempt restores the formerly current release.

Database migrations and secrets are preserved. Check backward schema compatibility
before rolling back a migrated release. Deployment snapshots are in
`/opt/xfchess/backups/pre-<release-id>/`; these are private, local recovery copies,
not an offsite backup. The first upgrade from the old layout saves a `legacy-*`
release and the original binary/website under `*.before-releases`.

For offsite database recovery, choose an explicit encrypted object and first test it:

```bash
export BACKUP_AGE_IDENTITY=/secure/offline-age-identity
export BACKUP_REMOTE=remote:bucket/db
bash /opt/xfchess/ops/backup/restore-db.sh sessions sessions-<timestamp>.db.age --check
sudo systemctl stop xfchess-backend
bash /opt/xfchess/ops/backup/restore-db.sh sessions sessions-<timestamp>.db.age
sudo systemctl start xfchess-backend
curl -fsS http://127.0.0.1:8090/readyz
curl -fsS http://127.0.0.1:8090/health
```

Restore refuses a running service, checks integrity, preserves the current database
under `data/recovery/`, clears old WAL/SHM files and atomically replaces the database
on the same filesystem. It asks before discarding later writes. Restoring several
databases requires selecting a compatible recovery point; separate snapshots do
not establish a transaction spanning both databases. Keep the matching vault
encryption secrets offline.
