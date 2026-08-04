#!/usr/bin/env bash
# XFChess DB restore — pull an encrypted snapshot, decrypt, and place it.
#
# Decryption needs the age PRIVATE key, which is kept OFFLINE (not on the server).
# Run this from a trusted operator machine that has the key, or copy the key in
# temporarily via `break-glass` and shred it afterwards.
#
# Usage:
#   BACKUP_AGE_IDENTITY=/path/to/age-key.txt \
#   ./restore-db.sh <db-name> [<object-name.age>] [<dest-path>]
#
#   db-name      e.g. sessions | vault        (matches <name>.db)
#   object-name  specific backup to restore   (default: latest for that db)
#   dest-path    where to write the .db        (default: /opt/xfchess/data/<name>.db)
#
# Env: BACKUP_REMOTE (rclone target, same as backup-db.sh), BACKUP_AGE_IDENTITY (key file)
set -euo pipefail

NAME="${1:?usage: restore-db.sh <db-name> [object.age] [dest]}"
OBJECT="${2:-}"
DEST="${3:-/opt/xfchess/data/${NAME}.db}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

log() { echo "[restore $(date -u +%H:%M:%S)] $*"; }
die() { echo "[restore ERROR] $*" >&2; exit 1; }

command -v age    >/dev/null || die "age not installed"
command -v rclone >/dev/null || die "rclone not installed"
[ -n "${BACKUP_REMOTE:-}" ]        || die "BACKUP_REMOTE not set"
[ -n "${BACKUP_AGE_IDENTITY:-}" ]  || die "BACKUP_AGE_IDENTITY (private key file) not set"
[ -f "$BACKUP_AGE_IDENTITY" ]      || die "identity file $BACKUP_AGE_IDENTITY not found"

if [ -z "$OBJECT" ]; then
  log "finding latest backup for '$NAME' in $BACKUP_REMOTE"
  OBJECT="$(rclone lsf "$BACKUP_REMOTE" --include "${NAME}-*.db.age" | sort | tail -n1)"
  [ -n "$OBJECT" ] || die "no backups found for $NAME"
fi
log "selected: $OBJECT"

rclone copyto "$BACKUP_REMOTE/$OBJECT" "$TMP/$OBJECT" || die "download failed"
age -d -i "$BACKUP_AGE_IDENTITY" -o "$TMP/${NAME}.db" "$TMP/$OBJECT" || die "decrypt failed"

# Integrity check before touching the live file.
sqlite3 "$TMP/${NAME}.db" "PRAGMA integrity_check;" | grep -q '^ok$' \
  || die "restored DB failed integrity_check — aborting"

if [ -f "$DEST" ]; then
  cp -a "$DEST" "${DEST}.pre-restore-$(date -u +%Y%m%dT%H%M%SZ)"
  log "backed up existing $DEST"
fi
log "STOP the backend before overwriting the live DB: sudo systemctl stop xfchess-backend"
read -r -p "Backend stopped? Overwrite $DEST now? [yes/NO] " ans
[ "$ans" = "yes" ] || die "aborted by operator"

mv "$TMP/${NAME}.db" "$DEST"
log "restored → $DEST  (start backend: sudo systemctl start xfchess-backend)"
