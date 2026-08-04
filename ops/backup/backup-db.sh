#!/usr/bin/env bash
# XFChess encrypted offsite DB backup.
#
# Takes a consistent snapshot of each SQLite DB (safe under WAL + live writes via
# `VACUUM INTO`), encrypts it with `age` (asymmetric — the server holds only the
# public recipient key and CANNOT decrypt its own backups), and uploads to an
# S3-compatible / SFTP remote via `rclone`. Old local snapshots are pruned.
#
# Runs as the xfchess-backup.timer (daily). Restore: see restore-db.sh + docs/DR.md.
#
# Required env (from /opt/xfchess/.env or the systemd unit):
#   BACKUP_AGE_RECIPIENT   age public key, e.g. age1qz...   (keep the PRIVATE key OFFLINE)
#   BACKUP_REMOTE          rclone target, e.g. r2:xfchess-backups/db
# Optional env:
#   BACKUP_DATA_DIR        default /opt/xfchess/data
#   BACKUP_WORK_DIR        default /opt/xfchess/data/backups
#   BACKUP_RETENTION_DAYS  default 14   (local copies; set bucket lifecycle for offsite)
set -euo pipefail

DATA_DIR="${BACKUP_DATA_DIR:-/opt/xfchess/data}"
WORK_DIR="${BACKUP_WORK_DIR:-/opt/xfchess/data/backups}"
RETENTION_DAYS="${BACKUP_RETENTION_DAYS:-14}"
STAMP="$(date -u +%Y%m%dT%H%M%SZ)"

log() { echo "[backup $(date -u +%H:%M:%S)] $*"; }
die() { echo "[backup ERROR] $*" >&2; exit 1; }

command -v sqlite3 >/dev/null || die "sqlite3 not installed (apt-get install sqlite3)"
command -v age      >/dev/null || die "age not installed (apt-get install age)"
command -v rclone   >/dev/null || die "rclone not installed (https://rclone.org/install)"
[ -n "${BACKUP_AGE_RECIPIENT:-}" ] || die "BACKUP_AGE_RECIPIENT not set"
[ -n "${BACKUP_REMOTE:-}" ]        || die "BACKUP_REMOTE not set"
[ -d "$DATA_DIR" ] || die "data dir $DATA_DIR not found"

mkdir -p "$WORK_DIR"
shopt -s nullglob
dbs=("$DATA_DIR"/*.db)
[ ${#dbs[@]} -gt 0 ] || die "no *.db files in $DATA_DIR"

for db in "${dbs[@]}"; do
  name="$(basename "$db" .db)"
  snap="$WORK_DIR/${name}-${STAMP}.db"
  enc="${snap}.age"

  log "snapshotting $name → $(basename "$snap")"
  # VACUUM INTO takes a consistent copy without blocking writers.
  sqlite3 "$db" "VACUUM INTO '$snap';" || die "snapshot failed for $name"

  log "encrypting → $(basename "$enc")"
  age -r "$BACKUP_AGE_RECIPIENT" -o "$enc" "$snap" || die "encrypt failed for $name"
  rm -f "$snap"   # never keep the plaintext snapshot

  log "uploading → $BACKUP_REMOTE/$(basename "$enc")"
  rclone copyto "$enc" "$BACKUP_REMOTE/$(basename "$enc")" || die "upload failed for $name"
done

# Prune old local encrypted copies (offsite retention handled by bucket lifecycle).
find "$WORK_DIR" -name '*.age' -type f -mtime "+$RETENTION_DAYS" -delete || true

log "done — ${#dbs[@]} database(s) backed up to $BACKUP_REMOTE"
