#!/bin/bash
# XFChess Monitoring Stack Setup - Local Development
# Run this on your local machine

set -e

echo "=========================================="
echo "XFChess Monitoring Stack (Local)"
echo "=========================================="

# Check if Docker is installed
if ! command -v docker &> /dev/null; then
    echo "Docker not found. Please install Docker Desktop first."
    exit 1
fi

# Check if Docker Compose is installed
if ! command -v docker-compose &> /dev/null; then
    echo "Docker Compose not found. Installing..."
    # Docker Compose is included in Docker Desktop
    exit 1
fi

# Create directories
echo "Creating directories..."
mkdir -p deploy/grafana/{datasources,dashboards}

# Start monitoring stack
echo "Starting monitoring stack..."
cd deploy
docker-compose -f docker-compose.monitoring.local.yml up -d

echo ""
echo "=========================================="
echo "Monitoring Stack Started!"
echo "=========================================="
echo ""
echo "Access your dashboards:"
echo "  Grafana:        http://localhost:3000"
echo "  Prometheus:     http://localhost:9090"
echo ""
echo "Default Grafana credentials:"
echo "  Username: admin"
echo "  Password: admin"
echo ""
echo "Backend health endpoints (ensure backend is running):"
echo "  Basic health:   http://localhost:8090/health"
echo "  Detailed health: http://localhost:8090/health/detailed"
echo "  Metrics:         http://localhost:8090/metrics"
echo ""
echo "To view logs:"
echo "  docker-compose -f monitoring/docker-compose.local.yml logs -f"
echo ""
echo "To stop monitoring:"
echo "  docker-compose -f monitoring/docker-compose.local.yml down"
echo "=========================================="
