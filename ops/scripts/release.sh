#!/usr/bin/env bash
# shellcheck disable=SC2034
set -Eeuo pipefail
umask 077
ROOT=/opt/xfchess
SERVICE=xfchess-backend
die() { echo "$*" >&2; exit 1; }

lock_release() {
  [[ $EUID == 0 ]] || die 'Release operations require root; the deploy console user cannot provision or deploy.'
  mkdir -p "$ROOT/releases" "$ROOT/backups"
  exec 9>"$ROOT/.release.lock"
  flock -n 9 || die 'Another release operation is running.'
}

switch_release() {
  local release=$1
  [[ $release =~ ^[a-zA-Z0-9_-]+$ && -x "$ROOT/releases/$release/signing-server-http" && -f "$ROOT/releases/$release/web/index.html" ]] || die 'Incomplete release.'
  ln -s "releases/$release" "$ROOT/current.new"
  mv -Tf "$ROOT/current.new" "$ROOT/current"
}

verify_release() {
  local expected=$1 health
  for ((attempt=0; attempt<30; attempt++)); do
    if curl --fail --silent --max-time 2 http://127.0.0.1:8090/readyz >/dev/null &&
       health=$(curl --fail --silent --max-time 2 http://127.0.0.1:8090/health) &&
       python3 -c 'import json,sys; h=json.loads(sys.argv[1]); sha=sys.argv[2]; got=h.get("git_sha", ""); sys.exit(0 if h.get("status")=="ok" and (sha=="legacy" or (len(got)>=7 and sha.startswith(got))) else 1)' "$health" "$expected"; then
      return 0
    fi
    sleep 2
  done
  return 1
}

config_files=(/etc/nginx/snippets/xfchess-routes.conf /etc/nginx/snippets/xfchess-proxy.conf /etc/nginx/conf.d/xfchess_rate_limit.conf /etc/nginx/sites-available/xfchess /etc/systemd/system/xfchess-backend.service)

save_config() {
  local dest=$1 file
  mkdir -p "$dest"
  for file in "${config_files[@]}"; do
    if [[ -f $file ]]; then cp --parents "$file" "$dest"; fi
  done
}

restore_config() {
  local from=$1 file
  for file in "${config_files[@]}"; do
    if [[ -f "$from$file" ]]; then cp "$from$file" "$file"; else rm -f "$file"; fi
  done
  systemctl daemon-reload
  nginx -t
  systemctl reload nginx
}

install_config() {
  local release=$1 domain=$2 src="$ROOT/releases/$1/src/ops"
  install -d /etc/nginx/snippets /etc/nginx/conf.d
  install -m 644 "$src/nginx/routes.conf" /etc/nginx/snippets/xfchess-routes.conf
  install -m 644 "$src/nginx/proxy.conf" /etc/nginx/snippets/xfchess-proxy.conf
  install -m 644 "$src/nginx/xfchess_rate_limit.conf" /etc/nginx/conf.d/xfchess_rate_limit.conf
  sed "s/YOUR_DOMAIN/$domain/g" "$src/nginx/nginx.conf" >/etc/nginx/sites-available/xfchess
  install -m 644 "$src/backend/xfchess-backend.service" /etc/systemd/system/xfchess-backend.service
  systemctl daemon-reload
  nginx -t
}
