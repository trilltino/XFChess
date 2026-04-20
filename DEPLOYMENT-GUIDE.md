# XFChess Hetzner Deployment Guide

**Server:** 178.104.55.19 (Hetzner)  
**Frontend:** Built ✓  
**Scripts:** Ready ✓

## Quick Start (3 Steps)

### Step 1: Upload Frontend

Double-click `upload-to-hetzner.bat` or run in PowerShell:
```powershell
scp -r C:\Users\isich\XFChess\web-solana\dist\* root@178.104.55.19:/opt/xfchess/frontend/
```

### Step 2: SSH to Server & Run Setup

```bash
ssh root@178.104.55.19

# On server - run the deployment script
cd /root
bash deploy-to-hetzner.sh
```

### Step 3: Configure Keys & Start

Edit the environment file:
```bash
nano /opt/xfchess/backend/.env
```

Update these lines with your actual keys:
```
VPS_AUTHORITY_KEY=your_vps_authority_base58_key
KYC_AUTHORITY_KEY=your_kyc_authority_base58_key
FEE_PAYER_KEYS=your_fee_payer_key
SENDGRID_API_KEY=SG.your_sendgrid_api_key
```

Start the service:
```bash
systemctl start xfchess-backend
systemctl status xfchess-backend
```

## Verify Deployment

```bash
# Test health endpoint
curl http://178.104.55.19/health
curl http://178.104.55.19/api/health

# Check logs
journalctl -u xfchess-backend -f
```

Visit: **http://178.104.55.19**

## What's Already Done

✅ Frontend built (dist/ folder ready)  
✅ Production .env configured  
✅ Upload script created  
✅ Server deployment script created  

## What Happens on the Server

The `deploy-to-hetzner.sh` script will:

1. **Install dependencies** (nginx, rust, sqlite, ufw)
2. **Generate secure secrets** (API key, JWT, encryption keys)
3. **Create directories** (`/opt/xfchess/...`)
4. **Build the backend** (Rust binary)
5. **Configure nginx** (reverse proxy)
6. **Setup firewall** (block 8090, allow 80/22)
7. **Create systemd service** (auto-restart, non-root user)
8. **Setup backups** (daily at 3 AM)

## Security Features

- **Non-root service user** (`xfchess`)
- **Restricted .env permissions** (600 - owner only)
- **Database directory** (700 - owner only)
- **Nginx proxy** (no direct backend access)
- **Firewall** (UFW blocking port 8090 externally)
- **Security headers** (X-Frame-Options, XSS protection)
- **Systemd hardening** (ProtectSystem, NoNewPrivileges)

## Generating Solana Keys

If you need new keys, SSH to server and run:

```bash
# Generate VPS authority
solana-keygen new --outfile ~/vps-authority.json --no-passphrase
cat ~/vps-authority.json
# Copy the base58 private key to .env

# Generate KYC authority
solana-keygen new --outfile ~/kyc-authority.json --no-passphrase
cat ~/kyc-authority.json

# Generate fee payer
solana-keygen new --outfile ~/fee-payer.json --no-passphrase
cat ~/fee-payer.json
```

**IMPORTANT:** These keys control your Solana program. Keep them secret and back them up securely!

## Troubleshooting

### Can't SSH to server
```bash
# Test connection
ssh -v root@178.104.55.19

# Check if server is up
ping 178.104.55.19
```

### Backend won't start
```bash
# Check logs
journalctl -u xfchess-backend -n 50

# Common issues:
# 1. Missing keys in .env
# 2. Permission issues (check chown/chmod)
# 3. Database locked (restart service)
```

### Permission denied errors
```bash
# Fix permissions
chown -R xfchess:xfchess /opt/xfchess
chmod 600 /opt/xfchess/backend/.env
chmod 700 /opt/xfchess/data
systemctl restart xfchess-backend
```

### Frontend not loading
```bash
# Check nginx
cat /var/log/nginx/error.log
nginx -t
systemctl restart nginx

# Check files exist
ls -la /opt/xfchess/frontend/
```

## Maintenance Commands

```bash
# Restart backend
systemctl restart xfchess-backend

# View logs
journalctl -u xfchess-backend -f

# View nginx logs
tail -f /var/log/nginx/access.log
tail -f /var/log/nginx/error.log

# Manual backup
/opt/xfchess/backup.sh

# Update backend (after code changes)
cd /tmp/xfchess-build
git pull
cargo build --release --bin signing-server
cp target/release/signing-server /opt/xfchess/backend/
systemctl restart xfchess-backend
```

## Files on Server

```
/opt/xfchess/
├── backend/
│   ├── .env              # Secrets (600 permissions)
│   └── signing-server    # Rust binary
├── frontend/             # React build files
│   ├── index.html
│   └── assets/
├── data/                 # SQLite databases
│   ├── sessions.db
│   └── vault.db
└── backups/              # Daily backups
    ├── sessions_*.db
    └── vault_*.db
```

## API Endpoints

| Endpoint | Description |
|----------|-------------|
| `GET /health` | Health check |
| `GET /api/health` | Backend health |
| `POST /api/auth/register` | User registration |
| `POST /api/auth/login` | User login |
| `POST /api/identity/register` | KYC registration |
| `GET /api/identity/status/{pubkey}` | KYC status |
| `GET /api/tournaments` | List tournaments |

## Next Steps After Deployment

1. **Test wallet connection** on the site
2. **Register a test user** with KYC
3. **Create a test tournament**
4. **Monitor logs** for any issues
5. **Set up SSL** (optional, requires domain)

---

**Questions or issues?** Check the logs first:
```bash
journalctl -u xfchess-backend -n 100
```
