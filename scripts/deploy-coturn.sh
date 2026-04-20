#!/bin/bash
# Deploy coturn TURN server on Ubuntu/Debian VPS
# Can run on the same Hetzner VPS as the XFChess backend.
# Usage: ./deploy-coturn.sh [domain_or_ip] [turn_user] [turn_pass]
#
# Examples:
#   ./deploy-coturn.sh 178.104.55.19          # use Hetzner IP directly
#   ./deploy-coturn.sh turn.xfchess.com       # use a domain (requires DNS + certbot)

set -e

DOMAIN="${1:-178.104.55.19}"
TURN_USER="${2:-xfchess_turn}"
TURN_PASS="${3:-$(openssl rand -base64 32)}"

# Detect if DOMAIN is a raw IP address
IS_IP=false
if echo "$DOMAIN" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$'; then
    IS_IP=true
fi

# Shared secret for TURN authentication (more secure than long-term credentials)
SHARED_SECRET="$(openssl rand -hex 32)"

echo "=== XFChess TURN Server Deployment ==="
echo "Domain/IP: $DOMAIN"
echo "TURN User: $TURN_USER"
echo "Shared Secret: $SHARED_SECRET"
echo ""

# Update system
echo "[1/7] Updating system packages..."
apt-get update && apt-get upgrade -y

# Install coturn
echo "[2/7] Installing coturn..."
apt-get install -y coturn

# Install certbot for SSL (only needed when using a domain, not a raw IP)
echo "[3/7] Setting up SSL certificates..."
mkdir -p /etc/coturn/certs

if [ "$IS_IP" = "true" ]; then
    echo "IP-only deployment — generating self-signed certificate..."
    openssl req -x509 -newkey rsa:2048 \
        -keyout /etc/coturn/certs/turn_server_pkey.pem \
        -out    /etc/coturn/certs/turn_server_cert.pem \
        -days 3650 -nodes \
        -subj "/CN=$DOMAIN" 2>/dev/null
else
    apt-get install -y certbot
    if [ ! -f "/etc/letsencrypt/live/$DOMAIN/fullchain.pem" ]; then
        echo "Please ensure DNS A record points to this server, then run:"
        echo "  certbot certonly --standalone -d $DOMAIN"
        echo "Skipping certificate generation for now."
    else
        cp /etc/letsencrypt/live/$DOMAIN/fullchain.pem /etc/coturn/certs/turn_server_cert.pem
        cp /etc/letsencrypt/live/$DOMAIN/privkey.pem   /etc/coturn/certs/turn_server_pkey.pem
    fi
fi

# Configure coturn
echo "[5/7] Configuring coturn..."
cat > /etc/turnserver.conf <<EOF
# XFChess TURN Server Configuration
# Generated: $(date)

# Listening ports
listening-port=3478
tls-listening-port=5349

# Listening IPs (all interfaces)
listening-ip=0.0.0.0
relay-ip=0.0.0.0

# External IP — required so coturn advertises the correct public address.
# When running on the Hetzner VPS this should be 178.104.55.19.
external-ip=$DOMAIN

# Realm for TURN (can be an IP or a domain)
realm=$DOMAIN

# Authentication - use shared secret (more secure)
use-auth-secret
static-auth-secret=$SHARED_SECRET

# Long-term credentials kept as a fallback for testing
user=$TURN_USER:$TURN_PASS

# TLS/SSL certificates
cert=/etc/coturn/certs/turn_server_cert.pem
pkey=/etc/coturn/certs/turn_server_pkey.pem

# Relay port range
min-port=10000
max-port=20000

# Performance tuning
max-bps=1000000000
bps-capacity=1000000000

# Logging
log-file=/var/log/turnserver.log
simple-log
verbose

# Security settings
no-multicast-peers
no-loopback-peers
stale-nonce=600
total-quota=10000
user-quota=1000

# Rate limiting
max-allocate-lifetime=3600
min-allocate-lifetime=300

# CLI admin (optional, for monitoring)
# cli-ip=127.0.0.1
# cli-port=5766
# cli-password=<ADMIN_PASSWORD>
EOF

# Set proper permissions
chown -R turnserver:turnserver /etc/coturn
chmod 600 /etc/coturn/certs/*
chmod 644 /etc/turnserver.conf

# Configure firewall
echo "[6/7] Configuring firewall..."
if command -v ufw &> /dev/null; then
    ufw allow 3478/tcp
    ufw allow 3478/udp
    ufw allow 5349/tcp
    ufw allow 5349/udp
    ufw allow 10000:20000/udp
    echo "UFW rules added"
elif command -v iptables &> /dev/null; then
    iptables -I INPUT -p tcp --dport 3478 -j ACCEPT
    iptables -I INPUT -p udp --dport 3478 -j ACCEPT
    iptables -I INPUT -p tcp --dport 5349 -j ACCEPT
    iptables -I INPUT -p udp --dport 5349 -j ACCEPT
    iptables -I INPUT -p udp --dport 10000:20000 -j ACCEPT
    echo "iptables rules added (not persisted)"
fi

# Enable and start coturn service
echo "[7/7] Starting coturn service..."
systemctl enable coturn
systemctl restart coturn

# Wait for service to start
sleep 2

# Check if coturn is running
if systemctl is-active --quiet coturn; then
    echo ""
    echo "=== TURN Server Deployed Successfully ==="
    echo "Status: $(systemctl status coturn --no-pager -l | grep Active)"
    echo ""
    echo "Configuration Summary:"
    echo "  Server: $DOMAIN"
    echo "  Standard port: 3478 (udp/tcp)"
    echo "  TLS port: 5349 (tcp)"
    echo "  Relay ports: 10000-20000 (udp)"
    echo "  Realm: $DOMAIN"
    echo "  Shared Secret: $SHARED_SECRET"
    echo ""
    echo "Environment variables for XFChess (.env):"
    if [ "$IS_IP" = "true" ]; then
        echo "  TURN_SERVER=turns://$DOMAIN:5349?transport=tcp"
    else
        echo "  TURN_SERVER=turns://$DOMAIN:5349"
    fi
    echo "  TURN_USERNAME=$TURN_USER"
    echo "  TURN_PASSWORD=$TURN_PASS"
    echo "  TURN_REALM=$DOMAIN"
    echo "  TURN_SHARED_SECRET=$SHARED_SECRET"
    echo ""
    echo "Test command:"
    echo "  turnutils_uclient -v -u $TURN_USER -w $TURN_PASS $DOMAIN"
    echo ""
    echo "Logs: tail -f /var/log/turnserver.log"
else
    echo "ERROR: coturn failed to start"
    echo "Check logs: journalctl -u coturn -n 50"
    exit 1
fi
