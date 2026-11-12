#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"
source ./compose.sh
"${COMPOSE[@]}" -f docker-compose.local.yml config --quiet
"${COMPOSE[@]}" -f docker-compose.local.yml up -d --wait --wait-timeout 120
echo 'Grafana: http://127.0.0.1:3000; Prometheus: http://127.0.0.1:9090 (local only).'
