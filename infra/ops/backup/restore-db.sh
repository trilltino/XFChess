#!/usr/bin/env bash
set -euo pipefail
umask 077
name=${1:?usage: restore-db.sh NAME OBJECT.db.age [--check]}
object=${2:?Select an explicit backup object; do not infer a recovery point from latest.}
mode=${3:-restore}
data=${BACKUP_DATA_DIR:-/opt/xfchess/data}
service=${BACKUP_SERVICE:-xfchess-backend}
die() { echo "Restore: $*" >&2; exit 1; }
[[ $name =~ ^[a-zA-Z0-9_-]+$ && $object =~ ^[a-zA-Z0-9_-]+\.db\.age$ && $object == "$name-"* ]] || die 'Invalid database name or object.'
[[ $mode == restore || $mode == --check ]] || die 'Unknown restore option.'
for tool in sqlite3 age rclone flock; do command -v "$tool" >/dev/null || die "$tool is required"; done
[[ -n ${BACKUP_REMOTE:-} && -f ${BACKUP_AGE_IDENTITY:-} && -d $data ]] || die 'Set backup remote, identity and data directory.'
exec 9>"$data/.restore.lock"
flock -n 9 || die 'Another restore is running.'
tmp=$(mktemp -d "$data/.restore.XXXXXX")
trap 'rm -rf -- "$tmp"' EXIT
rclone copyto "${BACKUP_REMOTE%/}/$object" "$tmp/object.age"
age -d -i "$BACKUP_AGE_IDENTITY" -o "$tmp/restored.db" "$tmp/object.age"
[[ $(sqlite3 "$tmp/restored.db" 'PRAGMA integrity_check;') == ok ]] || die 'Snapshot integrity check failed.'
if [[ $mode == --check ]]; then echo 'Decryption and integrity check passed; database untouched.'; exit 0; fi
command -v systemctl >/dev/null || die 'systemctl is required for live recovery.'
[[ $(systemctl show "$service" -p ActiveState --value) == inactive ]] || die "Stop $service before restoring."
read -r -p "Replace $name with $object, losing later writes? Type restore: " answer
[[ $answer == restore ]] || die 'Cancelled.'
dest="$data/$name.db"
if [[ -f $dest ]]; then
  mkdir -p "$data/recovery"
  before="$data/recovery/$name.pre-restore-$(date -u +%Y%m%dT%H%M%S%NZ).db"
  sqlite3 "$dest" ".backup '$before'"
  [[ $(sqlite3 "$before" 'PRAGMA integrity_check;') == ok ]] || die 'Cannot preserve current database.'
  chown --reference="$dest" "$tmp/restored.db"
  chmod --reference="$dest" "$tmp/restored.db"
else
  chown "${BACKUP_RESTORE_OWNER:-xfchess:xfchess}" "$tmp/restored.db"
fi
[[ $(systemctl show "$service" -p ActiveState --value) == inactive ]] || die 'Service started during restore; aborting.'
rm -f -- "$dest-wal" "$dest-shm"
mv -f -- "$tmp/restored.db" "$dest"
echo "Restored $name. Start $service and verify /readyz and /health."
