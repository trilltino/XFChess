# XFChess Deployment

Modular deployment configuration for XFChess backend and monitoring stack.

## Directory Structure

```
deploy/
├── monitoring/          # Prometheus + Grafana + AlertManager
│   ├── docker-compose.yml              # Production monitoring stack
│   ├── docker-compose.local.yml        # Local development monitoring
│   ├── prometheus.yml                  # Prometheus config
│   ├── prometheus.local.yml            # Local Prometheus config
│   ├── alertmanager.yml                # AlertManager config
│   ├── grafana/                        # Grafana provisioning
│   │   ├── datasources/
│   │   └── dashboards/
│   ├── setup.sh                        # Production setup script
│   └── setup-local.sh                  # Local setup script
│
├── backend/             # Backend deployment
│   ├── xfchess-backend.service         # Systemd service file
│   ├── .env.example                    # Environment template
│   └── .env.production                 # Production environment
│
├── nginx/               # Nginx configuration
│   └── nginx.conf                      # Reverse proxy config
│
├── scripts/             # Deployment utilities
│   ├── deploy.ps1                      # Full deploy script (backend + frontend + nginx)
│   └── rollback.ps1                    # Rollback script
│
└── docs/                # Documentation
    ├── HOSTED_BACKEND_CHECKLIST.md     # Deployment checklist
    └── ROLLBACK_GUIDE.md               # Rollback instructions
```

## Quick Start

### Local Development (Monitoring Only)

```bash
cd deploy/monitoring
./setup-local.sh
```

Access at:
- Grafana: http://localhost:3000 (admin/admin)
- Prometheus: http://localhost:9090

### Production Deployment (Hetzner)

```bash
ssh root@178.104.55.19
cd /opt/xfchess/src/deploy/monitoring
./setup.sh
```

Access at:
- Grafana: http://178.104.55.19:3000 (admin/admin)
- Prometheus: http://178.104.55.19:9090
- Backend: http://178.104.55.19:8090

## Backend Deployment

See `docs/HOSTED_BACKEND_CHECKLIST.md` for full deployment instructions.

Quick deploy:
```bash
cd deploy/scripts
./deploy.ps1 -Server 178.104.55.19 -User root
```

## Monitoring Stack

The monitoring stack includes:
- **Prometheus**: Metrics collection and storage
- **Grafana**: Visualization dashboards
- **AlertManager**: Alert routing and notifications

### Health Endpoints

- Basic health: `http://<server>:8090/health`
- Detailed health: `http://<server>:8090/health/detailed`
- Metrics: `http://<server>:8090/metrics`
- Debug TX: `http://<server>:8090/api/debug/tx/{signature}`

### Alert Rules

Alerts configured in `monitoring/prometheus-rules/xfchess.yml`:
- High error rate (>10%)
- Transaction failure rate (>5%)
- RPC latency high (>2s P95)
- Fee payer low balance (<0.01 SOL)
- Backend down

## Rollback

See `docs/ROLLBACK_GUIDE.md` for rollback procedures.

Quick rollback:
```bash
cd deploy/scripts
./rollback.ps1
```

## Environment Variables

Copy `backend/.env.example` to `backend/.env.production` and configure:

- `JWT_SECRET` - JWT signing secret
- `IDENTITY_ENCRYPTION_KEY` - Identity vault encryption key
- `IDENTITY_SALT` - Identity vault salt
- `SOLANA_RPC_URL` - Solana RPC endpoint
- `ALLOWED_ORIGINS` - CORS allowed origins
- Database URLs for sessions and vault

## Nginx Configuration

The `nginx/nginx.conf` provides reverse proxy configuration for:
- Backend API (port 8090)
- Static file serving
- SSL/TLS termination
