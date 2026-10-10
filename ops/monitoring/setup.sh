#!/bin/bash
# Idempotent monitoring setup; preserves existing secrets.

set -e

echo "=========================================="
echo "XFChess Monitoring Stack Setup"
echo "=========================================="

# Check if Docker is installed
if ! command -v docker &> /dev/null; then
    echo "Docker not found. Installing Docker..."
    curl -fsSL https://get.docker.com -o get-docker.sh
    sh get-docker.sh
    rm get-docker.sh
    usermod -aG docker $USER
    echo "Docker installed. Please log out and back in, then re-run this script."
    exit 1
fi

# Install standalone Compose when absent; the plugin package may be unavailable
# for Ubuntu-packaged Docker without changing its package repository.
if ! command -v docker-compose &> /dev/null; then
    echo "Docker Compose not found. Installing static binary..."
    curl -fsSL "https://github.com/docker/compose/releases/latest/download/docker-compose-linux-$(uname -m)" -o /usr/local/bin/docker-compose
    chmod +x /usr/local/bin/docker-compose
fi

echo "Creating directories..."
mkdir -p /opt/xfchess/monitoring
mkdir -p /opt/xfchess/monitoring/grafana/{datasources,dashboards}
mkdir -p /opt/xfchess/monitoring/rules
mkdir -p /opt/xfchess/monitoring/secrets

# Copy configuration files (this repo's tracked files are the source of truth —
# re-running this script always brings the server back in sync with them).
echo "Copying configuration files..."
cp docker-compose.yml /opt/xfchess/monitoring/
cp prometheus.yml /opt/xfchess/monitoring/
cp alertmanager.yml /opt/xfchess/monitoring/
cp rules/*.yml /opt/xfchess/monitoring/rules/
cp -r grafana/* /opt/xfchess/monitoring/grafana/

ENV_FILE=/opt/xfchess/monitoring/.env
touch "$ENV_FILE"
chmod 600 "$ENV_FILE"

if ! grep -q '^GRAFANA_ADMIN_PASSWORD=' "$ENV_FILE"; then
    echo "GRAFANA_ADMIN_PASSWORD=$(openssl rand -hex 16)" >> "$ENV_FILE"
    echo "Generated a new GRAFANA_ADMIN_PASSWORD in $ENV_FILE."
fi
if ! grep -q '^TELEGRAM_BOT_TOKEN=' "$ENV_FILE"; then
    echo "TELEGRAM_BOT_TOKEN=REPLACE_ME" >> "$ENV_FILE"
    echo "ACTION REQUIRED: set TELEGRAM_BOT_TOKEN in $ENV_FILE (create a bot via @BotFather), then re-run this script."
fi
if ! grep -q '^TELEGRAM_CHAT_ID=' "$ENV_FILE"; then
    echo "TELEGRAM_CHAT_ID=REPLACE_ME" >> "$ENV_FILE"
    echo "ACTION REQUIRED: set TELEGRAM_CHAT_ID in $ENV_FILE (message your bot, then check https://api.telegram.org/bot<token>/getUpdates), then re-run this script."
fi

# docker-compose reads Grafana credentials from the adjacent .env.

# Telegram bot token: Alertmanager reads it from a file (bot_token_file), not
# an env var — it has no env-var interpolation of its own.
BOT_TOKEN=$(grep '^TELEGRAM_BOT_TOKEN=' "$ENV_FILE" | cut -d= -f2-)
printf '%s' "$BOT_TOKEN" > /opt/xfchess/monitoring/secrets/telegram_bot_token
chmod 600 /opt/xfchess/monitoring/secrets/telegram_bot_token

# Alertmanager lacks chat_id_file; substitute the placeholder in the deployed copy.
CHAT_ID=$(grep '^TELEGRAM_CHAT_ID=' "$ENV_FILE" | cut -d= -f2-)
sed -i "s/__TELEGRAM_CHAT_ID__/${CHAT_ID}/g" /opt/xfchess/monitoring/alertmanager.yml

echo "Starting monitoring stack..."
cd /opt/xfchess/monitoring
docker-compose -f docker-compose.yml up -d

echo ""
echo "=========================================="
echo "Monitoring Stack Started!"
echo "=========================================="
echo ""
echo "Dashboards are internal-only — reach them via SSH tunnel, not a public URL:"
echo "  ssh -L 3000:127.0.0.1:3000 -L 9090:127.0.0.1:9090 -L 9093:127.0.0.1:9093 deploy@<server>"
echo "  Grafana:      http://localhost:3000  (user: admin, password: see GRAFANA_ADMIN_PASSWORD in $ENV_FILE)"
echo "  Prometheus:   http://localhost:9090"
echo "  Alertmanager: http://localhost:9093"
echo ""
if grep -q 'REPLACE_ME' "$ENV_FILE"; then
    echo "WARNING: Telegram alerting is not fully configured yet — see the ACTION REQUIRED lines above."
    echo "         Alerts will fire in Alertmanager but won't be delivered until you fill in $ENV_FILE and re-run this script."
fi
echo ""
echo "Backend health endpoints (scraped by Prometheus, not for direct browsing):"
echo "  Basic health:    http://127.0.0.1:8090/health"
echo "  Detailed health: http://127.0.0.1:8090/health/detailed"
echo "  Metrics:         http://127.0.0.1:8090/metrics"
echo ""
echo "To view logs:    docker-compose -f /opt/xfchess/monitoring/docker-compose.yml logs -f"
echo "To stop:         docker-compose -f /opt/xfchess/monitoring/docker-compose.yml down"
echo "=========================================="
