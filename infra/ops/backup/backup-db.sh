#!/usr/bin/env bash
set -euo pipefail
umask 077
DATA_DIR=${BACKUP_DATA_DIR:-/opt/xfchess/data}
WORK_DIR=${BACKUP_WORK_DIR:-/opt/xfchess/data/backups}
RETENTION_DAYS=${BACKUP_RETENTION_DAYS:-14}
die() { echo "Backup: $*" >&2; exit 1; }
for tool in sqlite3 age rclone flock; do command -v "$tool" >/dev/null || die "$tool is required"; done
[[ -n ${BACKUP_AGE_RECIPIENT:-} && -n ${BACKUP_REMOTE:-} ]] || die 'Set BACKUP_AGE_RECIPIENT and BACKUP_REMOTE.'
[[ $RETENTION_DAYS =~ ^[0-9]+$ && -d $DATA_DIR ]] || die 'Invalid retention or missing data directory.'
mkdir -p "$WORK_DIR"
exec 9>"$WORK_DIR/.backup.lock"
flock -n 9 || die 'Another backup is running.'
tmp=$(mktemp -d "$WORK_DIR/.snapshot.XXXXXX")
trap 'rm -rf -- "$tmp"' EXIT
stamp=$(date -u +%Y%m%dT%H%M%S%NZ)
shopt -s nullglob
dbs=("$DATA_DIR/"*.db)
[[ ${#dbs[@]} -gt 0 ]] || die 'No databases found.'
for db in "${dbs[@]}"; do
  name=$(basename "$db" .db)
  [[ $name =~ ^[a-zA-Z0-9_-]+$ ]] || die 'Unsupported database filename.'
  snapshot="$tmp/$name.db"
  encrypted="$WORK_DIR/$name-$stamp.db.age"
  sqlite3 "$db" '.timeout 10000' ".backup '$snapshot'"
  [[ $(sqlite3 "$snapshot" 'PRAGMA integrity_check;') == ok ]] || die "Invalid snapshot: $name"
  age -r "$BACKUP_AGE_RECIPIENT" -o "$encrypted" "$snapshot"
  rm -f -- "$snapshot"
  rclone copyto "$encrypted" "${BACKUP_REMOTE%/}/$(basename "$encrypted")"
done
find "$WORK_DIR" -maxdepth 1 -name '*.db.age' -type f -mtime "+$RETENTION_DAYS" -delete
echo "Encrypted and uploaded ${#dbs[@]} database snapshots."
