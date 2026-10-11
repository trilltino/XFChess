#!/usr/bin/env bash
set -Eeuo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/release.sh"
release=${1:?release} sha=${2:?commit} domain=${3:?domain} monitor=${4:-0}
[[ $release =~ ^[a-zA-Z0-9_-]+$ && $sha =~ ^[a-f0-9]{40}$ && $domain =~ ^[a-zA-Z0-9][a-zA-Z0-9.-]+$ ]] || die 'Invalid release arguments.'
lock_release
[[ -f "$ROOT/.env" && -d "$ROOT/data" ]] || die 'Provision the server and configure /opt/xfchess/.env first.'
[[ -f "/etc/letsencrypt/live/$domain/fullchain.pem" ]] || die 'Provision a trusted TLS certificate before deploying.'
[[ -L /etc/nginx/sites-enabled/xfchess ]] || die 'Enable the XFChess nginx site first (see ops/README.md).'
incoming="$ROOT/incoming/$release"
dest="$ROOT/releases/$release"
[[ ! -e $dest ]] || die 'Release already exists; inspect its state before retrying.'
mkdir -p "$dest/src" "$dest/web"
tar -xzf "$incoming/source.tar.gz" -C "$dest/src"
tar -xzf "$incoming/web.tar.gz" -C "$dest/web"
[[ -f "$dest/web/index.html" ]] || die 'Frontend index missing.'
printf '%s\n' "$sha" >"$dest/commit"
printf '%s\n' "$domain" >"$dest/domain"
chown -R xfchess:xfchess "$dest/src"
chmod 755 "$ROOT/releases" "$dest" "$dest/src"
export CARGO_HOME="$ROOT/.cargo" RUSTUP_HOME="$ROOT/.rustup" HOME="$ROOT"
export PATH="$CARGO_HOME/bin:$PATH" CARGO_TARGET_DIR="$ROOT/build-target" XFCHESS_BUILD_SHA="$sha"
install -d -o xfchess -g xfchess "$CARGO_TARGET_DIR"
# $1 is expanded by the unprivileged build shell.
# shellcheck disable=SC2016
timeout 2400s runuser -u xfchess -- env CARGO_HOME="$CARGO_HOME" RUSTUP_HOME="$RUSTUP_HOME" HOME="$HOME" PATH="$PATH" CARGO_TARGET_DIR="$CARGO_TARGET_DIR" XFCHESS_BUILD_SHA="$sha" \
  bash -c 'cd "$1" && cargo build --locked --release -p backend --bin signing-server-http' bash "$dest/src" | tee "$dest/build.log"
install -m 755 "$CARGO_TARGET_DIR/release/signing-server-http" "$dest/signing-server-http"
chmod -R a+rX "$dest/web"

snapshot="$ROOT/backups/pre-$release"
mkdir -m 700 "$snapshot"
shopt -s nullglob
dbs=("$ROOT/data/"*.db)
if [[ -f $ROOT/signing-server-http && ${#dbs[@]} -eq 0 ]]; then die 'No database files in data/; check configured database paths.'; fi
for db in "${dbs[@]}"; do
  [[ $(basename "$db") =~ ^[a-zA-Z0-9_-]+\.db$ ]] || die 'Unsupported database filename.'
  sqlite3 "$db" ".timeout 10000" ".backup '$snapshot/$(basename "$db")'"
  [[ $(sqlite3 "$snapshot/$(basename "$db")" 'PRAGMA integrity_check;') == ok ]] || die 'Snapshot integrity check failed.'
done

if [[ -L "$ROOT/current" ]]; then
  previous=$(basename "$(readlink "$ROOT/current")")
elif [[ -f "$ROOT/signing-server-http" && -f "$ROOT/web/index.html" ]]; then
  previous="legacy-$(date -u +%Y%m%dT%H%M%SZ)"
  baseline="$ROOT/releases/$previous"
  mkdir -p "$baseline"
  cp "$ROOT/signing-server-http" "$baseline/signing-server-http"
  cp -a "$ROOT/web" "$baseline/web"
  chmod 755 "$baseline" "$baseline/signing-server-http"
  printf 'legacy\n' >"$baseline/commit"
  save_config "$baseline/config"
  switch_release "$previous"
  mv "$ROOT/web" "$ROOT/web.before-releases"
  ln -s current/web "$ROOT/web"
  mv "$ROOT/signing-server-http" "$ROOT/signing-server-http.before-releases"
  ln -s current/signing-server-http "$ROOT/signing-server-http"
else
  previous=''
  [[ ! -e "$ROOT/signing-server-http" && ! -e "$ROOT/web" ]] || die 'Incomplete legacy installation; inspect existing files first.'
  ln -s current/web "$ROOT/web"
  ln -s current/signing-server-http "$ROOT/signing-server-http"
fi

save_config "$dest/config-before"
recover() {
  trap - ERR
  echo "Release failed; restoring $previous. Database migrations are not reversed." >&2
  if [[ -n $previous ]]; then switch_release "$previous"; else systemctl stop "$SERVICE"; fi
  restore_config "$dest/config-before"
  if [[ -n $previous ]]; then
    systemctl restart "$SERVICE"
    verify_release "$(cat "$ROOT/releases/$previous/commit")" || echo 'Previous release is unhealthy; operator intervention required.' >&2
  fi
  exit 1
}
trap recover ERR
install_config "$release" "$domain"
switch_release "$release"
systemctl restart "$SERVICE"
systemctl reload nginx
verify_release "$sha"
curl --fail --silent --max-time 10 "https://$domain/readyz" >/dev/null
save_config "$dest/config"
if [[ -n $previous ]]; then ln -sfn "releases/$previous" "$ROOT/previous"; fi
if [[ -e "$ROOT/ops" && ! -L "$ROOT/ops" ]]; then mv "$ROOT/ops" "$ROOT/ops.before-$release"; fi
# Keep recovery tools available even when rolling the application back to legacy.
ln -s "releases/$release/src/ops" "$ROOT/ops.new"
mv -Tf "$ROOT/ops.new" "$ROOT/ops"
install -m 644 "$dest/src/ops/backup/xfchess-backup.service" /etc/systemd/system/xfchess-backup.service
install -m 644 "$dest/src/ops/backup/xfchess-backup.timer" /etc/systemd/system/xfchess-backup.timer
systemctl daemon-reload
if grep -q '^BACKUP_AGE_RECIPIENT=age1' "$ROOT/.env" && grep -q '^BACKUP_REMOTE=' "$ROOT/.env"; then
  systemctl enable --now xfchess-backup.timer
fi
trap - ERR
if [[ $monitor == 1 ]] && ! bash "$dest/src/ops/monitoring/setup.sh"; then
  die "Application release $release is active and verified, but the monitoring update failed."
fi
echo "Release $release verified. Previous release: $previous"
