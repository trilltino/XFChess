#!/usr/bin/env bash
# shellcheck disable=SC2034
if docker compose version >/dev/null 2>&1; then
  COMPOSE=(docker compose)
elif command -v docker-compose >/dev/null && [[ $(docker-compose version --short) =~ ^[2-9][0-9]*\. ]]; then
  COMPOSE=(docker-compose)
else
  echo 'Install Docker Compose v2 or newer (plugin or standalone); setup does not install unverified binaries.' >&2
  exit 1
fi
