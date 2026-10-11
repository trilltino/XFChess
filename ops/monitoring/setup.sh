#!/usr/bin/env bash
set -euo pipefail
umask 077
[[ $EUID == 0 ]] || { echo 'Production monitoring setup requires root.' >&2; exit 1; }
source_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
source "$source_dir/compose.sh"
dest=/opt/xfchess/monitoring
install -d -m 755 "$dest"
touch "$dest/.env"
chmod 600 "$dest/.env"
if ! grep -q '^GRAFANA_ADMIN_PASSWORD=' "$dest/.env"; then
  printf 'GRAFANA_ADMIN_PASSWORD=%s\n' "$(openssl rand -hex 24)" >>"$dest/.env"
fi
# Validate staged files before replacing configuration mounted by running containers.
stage=$(mktemp -d /opt/xfchess/.monitoring.XXXXXX)
trap 'rm -rf -- "$stage"' EXIT
chmod 755 "$stage"
cp "$dest/.env" "$stage/.env"
cp "$source_dir/"{docker-compose.yml,prometheus.yml,alertmanager.yml} "$stage/"
cp -R "$source_dir/"{rules,grafana} "$stage/"
chmod 644 "$stage/"*.yml
chmod -R a+rX "$stage/rules" "$stage/grafana"
install -d -m 755 "$stage/secrets"
touch "$stage/secrets/telegram_bot_token"
chown root:65534 "$stage/secrets/telegram_bot_token"
chmod 640 "$stage/secrets/telegram_bot_token"
python3 "$source_dir/configure_alerts.py" "$stage"
cd "$stage"
"${COMPOSE[@]}" -p monitoring config --quiet
"${COMPOSE[@]}" -p monitoring run --rm --no-deps --entrypoint promtool prometheus check config /etc/prometheus/prometheus.yml
"${COMPOSE[@]}" -p monitoring run --rm --no-deps --entrypoint amtool alertmanager check-config /etc/alertmanager/config.yml
cp "$stage/"{docker-compose.yml,prometheus.yml,alertmanager.yml} "$dest/"
cp -R "$stage/"{rules,grafana,secrets} "$dest/"
chmod 644 "$dest/"*.yml
chmod -R a+rX "$dest/rules" "$dest/grafana"
chown root:65534 "$dest/secrets/telegram_bot_token"
chmod 640 "$dest/secrets/telegram_bot_token"
cd "$dest"
"${COMPOSE[@]}" -p monitoring up -d --wait --wait-timeout 120
echo 'Monitoring ready on loopback. Forward 3000/9090 using the deploy SSH user.'
echo 'Grafana password remains in /opt/xfchess/monitoring/.env; no credentials printed.'
