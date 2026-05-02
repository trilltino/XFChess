# XFChess Observability System

Complete observability implementation for XFChess backend and client.

## Quick Start

### Local Development

```bash
cd deploy/monitoring
./setup-local.sh
```

Access dashboards:
- **Grafana**: http://localhost:3000 (admin/admin)
- **Prometheus**: http://localhost:9090
- **Backend Health**: http://localhost:8090/health

### Production (Hetzner)

```bash
ssh root@178.104.55.19
cd /opt/xfchess/src/deploy/monitoring
./setup.sh
```

Access dashboards:
- **Grafana**: http://178.104.55.19:3000 (admin/admin)
- **Prometheus**: http://178.104.55.19:9090
- **Backend Health**: http://178.104.55.19:8090/health

## Environment Support

The observability system works in both environments:

| Environment | Docker Compose File | Prometheus Config | Backend URL |
|-------------|-------------------|------------------|-------------|
| **Local** | `docker-compose.monitoring.local.yml` | `prometheus.local.yml` | `host.docker.internal:8090` |
| **Production** | `docker-compose.monitoring.yml` | `prometheus.yml` | `178.104.55.19:8090` |

### Key Differences
- **Local**: 7-day data retention, localhost access
- **Production**: 30-day data retention, Hetzner server access, AlertManager enabled

## What Was Implemented

### Backend (Hetzner)

**New Files:**
- `backend/src/telemetry/` - Core telemetry module
- `backend/src/signing/solana/telemetry.rs` - Transaction telemetry
- `backend/src/signing/solana/debug.rs` - Transaction debugging
- `backend/src/signing/routes/debug.rs` - Health/debug endpoints

**New Endpoints:**
- `GET /health` - Basic health check
- `GET /health/detailed` - Full system health
- `GET /metrics` - Prometheus metrics
- `GET /api/debug/tx/{signature}` - Transaction debug info

**Deployment:**
- `deploy/docker-compose.monitoring.yml` - Prometheus + Grafana
- `deploy/setup-monitoring.sh` - One-command setup

### Client (Game)

**New Files:**
- `src/crash_reporter/` - Enhanced crash reporting
- Better panic hook with structured context

## Key Features

1. **Transaction Debugging**: See exactly why any Solana transaction failed
2. **Metrics Dashboard**: Real-time API performance and game stats
3. **Health Checks**: Database, RPC, fee payer monitoring
4. **Crash Reports**: Structured client crash logs
5. **Zero Subscriptions**: Everything self-hosted on your Hetzner VPS

## Next Steps

1. Deploy monitoring stack on Hetzner
2. Build backend with new telemetry features
3. Test endpoints: `curl http://178.104.55.19:8090/health`
4. View Grafana dashboard
5. Monitor transaction success rates
