#!/usr/bin/env bash
# Snapshot SQLite with VACUUM INTO, encrypt with age, and upload via rclone.
# Required: BACKUP_AGE_RECIPIENT, BACKUP_REMOTE; keep the private age key offline.
# Restore with restore-db.sh; see docs/DR.md.
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
