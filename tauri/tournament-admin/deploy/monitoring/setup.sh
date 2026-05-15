#!/bin/bash
# XFChess Monitoring Stack Setup Script
# Run this on your Hetzner server to set up Prometheus + Grafana

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

# Check if Docker Compose is installed
if ! command -v docker-compose &> /dev/null; then
    echo "Docker Compose not found. Installing..."
    apt-get update
    apt-get install -y docker-compose-plugin
fi

# Create directories
echo "Creating directories..."
mkdir -p /opt/xfchess/monitoring
mkdir -p /opt/xfchess/monitoring/grafana/{datasources,dashboards}
mkdir -p /opt/xfchess/monitoring/prometheus-rules

# Copy configuration files
echo "Copying configuration files..."
cp docker-compose.monitoring.yml /opt/xfchess/monitoring/
cp prometheus.yml /opt/xfchess/monitoring/
cp alertmanager.yml /opt/xfchess/monitoring/
cp -r grafana/* /opt/xfchess/monitoring/grafana/

# Update prometheus.yml to use correct backend address
echo "Updating Prometheus configuration..."
sed -i "s/host.docker.internal/178.104.55.19/g" /opt/xfchess/monitoring/prometheus.yml

# Create alert rules
cat > /opt/xfchess/monitoring/prometheus-rules/xfchess.yml << 'EOF'
groups:
  - name: xfchess_alerts
    rules:
      - alert: HighErrorRate
        expr: rate(http_requests_total{status=~"5.."}[5m]) > 0.1
        for: 2m
        labels:
          severity: warning
        annotations:
          summary: "High error rate on XFChess backend"
          description: "Error rate is {{ $value | humanizePercentage }} over the last 5 minutes"
      
      - alert: TransactionFailureRate
        expr: rate(transactions_failed_total[5m]) / rate(transactions_submitted_total[5m]) > 0.05
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High transaction failure rate"
          description: "Transaction failure rate is {{ $value | humanizePercentage }}"
      
      - alert: RpcLatencyHigh
        expr: histogram_quantile(0.95, rate(solana_rpc_latency_bucket[5m])) > 2
        for: 3m
        labels:
          severity: warning
        annotations:
          summary: "Solana RPC latency is high"
          description: "P95 latency is {{ $value }}s"
      
      - alert: FeePayerLowBalance
        expr: feepayer_balance_lamports < 10000000
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "Fee payer balance is critically low"
          description: "Fee payer {{ $labels.key_index }} has {{ $value }} lamports"
      
      - alert: BackendDown
        expr: xfchess_health == 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "XFChess backend is down"
          description: "Backend health check failed"
EOF

# Start monitoring stack
echo "Starting monitoring stack..."
cd /opt/xfchess/monitoring
docker-compose -f docker-compose.yml up -d

echo ""
echo "=========================================="
echo "Monitoring Stack Started!"
echo "=========================================="
echo ""
echo "Access your dashboards:"
echo "  Grafana:        http://178.104.55.19:3000"
echo "  Prometheus:     http://178.104.55.19:9090"
echo "  AlertManager:   http://178.104.55.19:9093"
echo ""
echo "Default Grafana credentials:"
echo "  Username: admin"
echo "  Password: admin"
echo ""
echo "IMPORTANT: Change the Grafana password immediately!"
echo "  1. Visit http://178.104.55.19:3000"
echo "  2. Login with admin/admin"
echo "  3. Go to Admin > Users"
echo "  4. Change the admin password"
echo ""
echo "Backend health endpoints:"
echo "  Basic health:   http://178.104.55.19:8090/health"
echo "  Detailed health: http://178.104.55.19:8090/health/detailed"
echo "  Metrics:         http://178.104.55.19:8090/metrics"
echo "  Debug TX:        http://178.104.55.19:8090/api/debug/tx/{signature}"
echo ""
echo "To view logs:"
echo "  docker-compose -f /opt/xfchess/monitoring/docker-compose.yml logs -f"
echo ""
echo "To stop monitoring:"
echo "  docker-compose -f /opt/xfchess/monitoring/docker-compose.yml down"
echo "=========================================="
