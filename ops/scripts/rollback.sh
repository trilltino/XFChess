#!/usr/bin/env bash
set -Eeuo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/release.sh"
lock_release
target=${1:-previous}
if [[ $target == previous ]]; then
  [[ -L "$ROOT/previous" ]] || die 'No previous release available.'
  target=$(basename "$(readlink "$ROOT/previous")")
fi
[[ $target =~ ^[a-zA-Z0-9_-]+$ && -f "$ROOT/releases/$target/commit" && -d "$ROOT/releases/$target/config" ]] || die 'Invalid or incomplete rollback release.'
current=$(basename "$(readlink "$ROOT/current")")
save_config "$ROOT/releases/$current/config"
recover() {
  trap - ERR
  switch_release "$current"
  restore_config "$ROOT/releases/$current/config"
  systemctl restart "$SERVICE"
  echo 'Rollback failed; restored the former release. Inspect service health.' >&2
  exit 1
}
trap recover ERR
switch_release "$target"
restore_config "$ROOT/releases/$target/config"
systemctl restart "$SERVICE"
verify_release "$(cat "$ROOT/releases/$target/commit")"
ln -sfn "releases/$current" "$ROOT/previous"
trap - ERR
echo "Rolled back backend, frontend and server configuration to $target. Databases preserved."
