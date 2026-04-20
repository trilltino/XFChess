# Solana Blinks Deployment Guide (Hetzner VPS)

## Prerequisites

- Hetzner VPS (Ubuntu 22.04 recommended)
- Domain name pointing to VPS (e.g., xfchess.com)
- SSH access to VPS
- SSL certificate (Let's Encrypt recommended)

## Server Setup

### 1. Update System

```bash
ssh root@your-vps-ip
apt update && apt upgrade -y
```

### 2. Install Dependencies

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Install Node.js
curl -fsSL https://deb.nodesource.com/setup_18.x | bash -
apt install -y nodejs

# Install Nginx
apt install -y nginx

# Install PM2 (for Node.js process management)
npm install -g pm2

# Install Certbot for SSL
apt install -y certbot python3-certbot-nginx
```

### 3. Clone Repository

```bash
cd /opt
git clone https://github.com/your-repo/XFChess.git
cd XFChess
```

### 4. Build Backend

```bash
cd backend
cargo build --release
```

### 5. Build Frontend

```bash
cd ../web-solana
npm install
npm run build
```

### 6. Configure Environment Variables

Create `.env` in `web-solana/`:

```bash
VITE_BACKEND_URL=https://xfchess.com/api
VITE_MOONPAY_API_KEY=your_moonpay_key
VITE_MOONPAY_PUBLISHABLE_KEY=your_moonpay_publishable_key
VITE_TRANSAK_API_KEY=your_transak_key
VITE_BANXA_API_KEY=your_banxa_key
```

Create `.env` in `backend/`:

```bash
DATABASE_URL=sqlite:///sessions.db
PROGRAM_ID=FVPp29xDtMrh3CrTJNxDcbGRnMMKuUv2ntqkBRc1uDX
RPC_URL=https://api.devnet.solana.com
FEE_PAYER_KEYPAIR=/opt/XFChess/keys/fee_payer.json
```

### 7. Setup SSL Certificate

```bash
certbot --nginx -d xfchess.com -d www.xfchess.com
```

### 8. Configure Nginx

Edit `/etc/nginx/sites-available/xfchess`:

```nginx
server {
    listen 80;
    server_name xfchess.com www.xfchess.com;
    return 301 https://$server_name$request_uri;
}

server {
    listen 443 ssl http2;
    server_name xfchess.com www.xfchess.com;

    ssl_certificate /etc/letsencrypt/live/xfchess.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/xfchess.com/privkey.pem;
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers HIGH:!aNULL:!MD5;

    # Frontend
    location / {
        root /opt/XFChess/web-solana/dist;
        try_files $uri $uri/ /index.html;
    }

    # Backend API
    location /api/ {
        proxy_pass http://localhost:8090;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection 'upgrade';
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_cache_bypass $http_upgrade;
    }

    # Blinks discovery
    location /actions.json {
        root /opt/XFChess/web-solana/dist;
        add_header Content-Type application/json;
    }
}
```

Enable the site:

```bash
ln -s /etc/nginx/sites-available/xfchess /etc/nginx/sites-enabled/
rm /etc/nginx/sites-enabled/default
nginx -t
systemctl restart nginx
```

### 9. Setup Backend Service

Create `/etc/systemd/system/xfchess-backend.service`:

```ini
[Unit]
Description=XFChess Backend Service
After=network.target

[Service]
Type=simple
User=root
WorkingDirectory=/opt/XFChess/backend
Environment="RUST_LOG=info"
ExecStart=/opt/XFChess/backend/target/release/signing-server-http
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
```

Enable and start the service:

```bash
systemctl daemon-reload
systemctl enable xfchess-backend
systemctl start xfchess-backend
systemctl status xfchess-backend
```

### 10. Copy actions.json to Web Root

```bash
cp /opt/XFChess/web-solana/public/actions.json /opt/XFChess/web-solana/dist/
```

## Verify Deployment

### 1. Check Backend Health

```bash
curl http://localhost:8090/health
```

### 2. Check Frontend

```bash
curl https://xfchess.com
```

### 3. Check actions.json

```bash
curl https://xfchess.com/actions.json
```

Expected response:
```json
{
  "rules": [
    {
      "pathPattern": "/api/actions/tournament/*",
      "apiPath": "/api/actions/tournament/{id}"
    }
  ]
}
```

### 4. Test Blinks Endpoint

```bash
curl https://xfchess.com/api/actions/tournament/1
```

## Configure MoonPay/Transak/Banxa

### MoonPay

1. Sign up at https://business.moonpay.com/
2. Create an API key
3. Add your domain to the whitelist
4. Update `VITE_MOONPAY_API_KEY` and `VITE_MOONPAY_PUBLISHABLE_KEY` in `.env`
5. Rebuild frontend: `cd /opt/XFChess/web-solana && npm run build`

### Transak

1. Sign up at https://transak.com/
2. Create an API key
3. Add your domain to the whitelist
4. Update `VITE_TRANSAK_API_KEY` in `.env`
5. Rebuild frontend

### Banxa

1. Sign up at https://banxa.com/
2. Create an API key
3. Add your domain to the whitelist
4. Update `VITE_BANXA_API_KEY` in `.env`
5. Rebuild frontend

## Testing in Production

### 1. Create Test Tournament

```bash
curl -X POST https://xfchess.com/api/admin/tournament \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_ADMIN_TOKEN" \
  -d '{
    "id": 1,
    "name": "Test Tournament",
    "entry_fee_lamports": 500000000,
    "max_players": 8,
    "elo_min": 1000,
    "elo_max": 2000,
    "status": "open"
  }'
```

### 2. Test with Solana Blinks Inspector

1. Open https://inspector.solana.com/
2. Enter: `https://xfchess.com/api/actions/tournament/1`
3. Test the action flow
4. Sign and submit transaction

### 3. Test Funding Flow

1. Open: `https://xfchess.com/fund?wallet=YOUR_WALLET&amount=0.5`
2. Select provider
3. Complete funding

## Monitoring

### View Backend Logs

```bash
journalctl -u xfchess-backend -f
```

### View Nginx Logs

```bash
tail -f /var/log/nginx/access.log
tail -f /var/log/nginx/error.log
```

### Monitor Disk Space

```bash
df -h
```

### Monitor Memory

```bash
free -h
```

## Security Hardening

### 1. Configure Firewall

```bash
ufw allow 22/tcp
ufw allow 80/tcp
ufw allow 443/tcp
ufw enable
```

### 2. Disable Root SSH

Edit `/etc/ssh/sshd_config`:

```
PermitRootLogin no
```

Add a sudo user and restart SSH:

```bash
adduser xfchess
usermod -aG sudo xfchess
systemctl restart sshd
```

### 3. Setup Fail2Ban

```bash
apt install -y fail2ban
cp /etc/fail2ban/jail.conf /etc/fail2ban/jail.local
systemctl enable fail2ban
systemctl start fail2ban
```

## Troubleshooting

### Backend Won't Start

```bash
journalctl -u xfchess-backend -n 50
```

Check:
- Environment variables are set
- Database file exists and has correct permissions
- Port 8090 is not already in use

### Nginx 502 Bad Gateway

```bash
# Check if backend is running
systemctl status xfchess-backend

# Check backend logs
journalctl -u xfchess-backend -n 50

# Check nginx configuration
nginx -t
```

### SSL Certificate Issues

```bash
# Renew certificate
certbot renew

# Force renewal
certbot renew --force-renewal
```

### CORS Errors

Ensure nginx is configured with proper CORS headers or add CORS middleware to the backend.

## Scaling

### Horizontal Scaling

For high traffic, consider:
1. Load balancer (HAProxy or AWS ALB)
2. Multiple backend instances
3. Database migration from SQLite to PostgreSQL
4. Redis for caching

### Database Migration

```bash
# Install PostgreSQL
apt install -y postgresql

# Create database
sudo -u postgres psql
CREATE DATABASE xfchess;
\q

# Update DATABASE_URL in .env
DATABASE_URL=postgresql://user:password@localhost/xfchess
```

## Backup Strategy

### Database Backup

```bash
# Backup SQLite
cp /opt/XFChess/backend/sessions.db /backup/sessions.db.$(date +%Y%m%d)

# Backup PostgreSQL
pg_dump xfchess > /backup/xfchess.sql.$(date +%Y%m%d)
```

### Code Backup

```bash
# Pull latest changes
cd /opt/XFChess
git pull

# Rebuild
cd backend && cargo build --release
cd ../web-solana && npm run build

# Restart services
systemctl restart xfchess-backend
```

## Rollback Plan

If deployment fails:

```bash
# Revert to previous version
cd /opt/XFChess
git checkout <previous-commit>

# Rebuild
cd backend && cargo build --release
cd ../web-solana && npm run build

# Restart services
systemctl restart xfchess-backend
```

## Cost Optimization

- Use Hetzner CX22 or CX31 for production
- Monitor resource usage and scale down if needed
- Consider CDN for static assets (Cloudflare)
- Use spot instances if possible (for non-critical workloads)
