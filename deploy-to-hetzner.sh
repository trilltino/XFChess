#!/bin/bash
# XFChess Deployment Script for Hetzner
# Run this on your Hetzner server (root@178.104.55.19)

set -e

echo "=== XFChess Deployment Script ==="
echo "This script will set up the XFChess backend and frontend on your Hetzner server"
echo ""

# Configuration
APP_DIR="/opt/xfchess"
DATA_DIR="$APP_DIR/data"
BACKEND_DIR="$APP_DIR/backend"
FRONTEND_DIR="$APP_DIR/frontend"
BACKUP_DIR="$APP_DIR/backups"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if running as root
if [ "$EUID" -ne 0 ]; then
    log_error "Please run as root (use: sudo bash deploy-to-hetzner.sh)"
    exit 1
fi

# 1. System Updates and Dependencies
log_info "Updating system packages..."
apt update && apt upgrade -y

log_info "Installing dependencies..."
apt install -y \
    nginx \
    git \
    curl \
    build-essential \
    pkg-config \
    libssl-dev \
       sqlite3 \
    libsqlite3-dev \
    ufw \
    openssl \
    cron

# 2. Install Rust if not present
if ! command -v cargo &> /dev/null; then
    log_info "Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source ~/.cargo/env
else
    log_info "Rust already installed"
fi

# 3. Create User and Directories
log_info "Creating xfchess user and directories..."
if ! id "xfchess" &>/dev/null; then
    useradd -r -s /bin/false xfchess
fi

mkdir -p $BACKEND_DIR $FRONTEND_DIR $DATA_DIR $BACKUP_DIR
chown -R xfchess:xfchess $APP_DIR
chmod 750 $APP_DIR
chmod 700 $DATA_DIR

# 4. Generate Secrets
log_info "Generating secure secrets..."
ADMIN_API_KEY=$(openssl rand -hex 32)
JWT_SECRET=$(openssl rand -hex 32)
IDENTITY_ENCRYPTION_KEY=$(openssl rand -hex 32)
IDENTITY_SALT=$(openssl rand -hex 32)

log_info "Admin API Key: ${ADMIN_API_KEY:0:16}..."
log_info "JWT Secret: ${JWT_SECRET:0:16}..."
log_info "Encryption Key: ${IDENTITY_ENCRYPTION_KEY:0:16}..."

# 5. Create Environment File
cat > $BACKEND_DIR/.env << EOF
# Admin API Key
ADMIN_API_KEY=$ADMIN_API_KEY

# Signing Service URL
SIGNING_SERVICE_URL=http://178.104.55.19

# JWT Secret
JWT_SECRET=$JWT_SECRET

# Identity encryption (64 hex chars each)
IDENTITY_ENCRYPTION_KEY=$IDENTITY_ENCRYPTION_KEY
IDENTITY_SALT=$IDENTITY_SALT

# Database URLs
DATABASE_URL=sqlite://$DATA_DIR/sessions.db?mode=rwc
VAULT_DATABASE_URL=sqlite://$DATA_DIR/vault.db?mode=rwc

# Solana Configuration
SOLANA_RPC_URL=https://api.devnet.solana.com
PROGRAM_ID=FVPp29xDtMrh3CrTJNnxDcbGRnMMKuUv2ntqkBRc1uDX

# SendGrid API Key - REPLACE THIS
SENDGRID_API_KEY=SG.your-sendgrid-api-key-here

# Authorities - REPLACE THESE WITH YOUR KEYS
VPS_AUTHORITY_KEY=YOUR_VPS_AUTHORITY_BASE58_KEY_HERE
KYC_AUTHORITY_KEY=YOUR_KYC_AUTHORITY_BASE58_KEY_HERE

# Fee payer keys - REPLACE THIS
FEE_PAYER_KEYS=YOUR_FEE_PAYER_KEY_HERE

# Port
SIGNING_PORT=8090

# GitHub token for re-deploy (source this file then re-run the script)
GITHUB_TOKEN=$GITHUB_TOKEN
EOF

chmod 600 $BACKEND_DIR/.env
chown xfchess:xfchess $BACKEND_DIR/.env
log_info "Environment file created at $BACKEND_DIR/.env"

# 6. Install Solana CLI (for key generation/verification)
log_info "Installing Solana CLI..."
if ! command -v solana &> /dev/null; then
    sh -c "$(curl -sSfL https://release.solana.com/v1.18.0/install)"
    export PATH="$HOME/.local/share/solana/install/active_release/bin:$PATH"
fi

# 7. Build Backend
log_info "Building Rust backend..."

REPO_URL="https://github.com/trilltino/XFChess.git"
BRANCH="latest"

# Private repo — require a GitHub PAT
if [ -z "$GITHUB_TOKEN" ]; then
    log_error "GITHUB_TOKEN is not set. Export it before running this script:"
    log_error "  export GITHUB_TOKEN=github_pat_xxxx"
    log_error "You can create a PAT at: https://github.com/settings/tokens"
    log_error "(Required permission: Contents: Read on the XFChess repository)"
    exit 1
fi

AUTH_REPO_URL="https://${GITHUB_TOKEN}@github.com/trilltino/XFChess.git"

if [ -d "/tmp/xfchess-build/.git" ]; then
    log_info "Repository already cloned — pulling latest changes from branch $BRANCH..."
    cd /tmp/xfchess-build
    git remote set-url origin "$AUTH_REPO_URL"
    git fetch origin
    # Use refs/heads/ prefix to disambiguate branch from tag with same name
    git checkout refs/heads/$BRANCH
    git reset --hard origin/$BRANCH
    # Strip PAT from remote to avoid it sitting on disk
    git remote set-url origin "$REPO_URL"
else
    if [ -d "/tmp/xfchess-build" ]; then
        rm -rf /tmp/xfchess-build
    fi
    log_info "Cloning repository (branch: $BRANCH)..."
    git clone --branch $BRANCH --depth 1 "$AUTH_REPO_URL" /tmp/xfchess-build
    cd /tmp/xfchess-build
    # Strip PAT from git config on disk immediately after clone
    git remote set-url origin "$REPO_URL"
fi

cd /tmp/xfchess-build/backend

# Ensure cargo is on PATH (needed on fresh installs where source ran in a subshell)
export PATH="$HOME/.cargo/bin:$PATH"

# Build the backend (standalone workspace — does not require Bevy/Tauri/Solana programs)
log_info "Compiling signing-server (this may take several minutes)..."
cargo build --release --bin signing-server 2>&1 | tee /tmp/build.log

# Check if build succeeded
if [ ! -f "target/release/signing-server" ]; then
    log_error "Build failed! Check /tmp/build.log"
    exit 1
fi

# Copy binary
cp target/release/signing-server $BACKEND_DIR/
chmod +x $BACKEND_DIR/signing-server
chown xfchess:xfchess $BACKEND_DIR/signing-server
log_info "Backend binary installed"

# 8. Nginx Configuration
cat > /etc/nginx/sites-available/xfchess << 'EOF'
server {
    listen 80;
    server_name 178.104.55.19;

    # Security headers
    add_header X-Frame-Options "SAMEORIGIN" always;
    add_header X-Content-Type-Options "nosniff" always;
    add_header X-XSS-Protection "1; mode=block" always;
    add_header Referrer-Policy "strict-origin-when-cross-origin" always;

    # Frontend - static files
    location / {
        root /opt/xfchess/frontend;
        index index.html;
        try_files $uri $uri/ /index.html;
        
        # Cache static assets
        location ~* \.(js|css|png|jpg|jpeg|gif|ico|svg|woff|woff2)$ {
            expires 1y;
            add_header Cache-Control "public, immutable";
        }
    }

    # Backend API - proxy to signing-server
    location /api/ {
        proxy_pass http://127.0.0.1:8090/;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection 'upgrade';
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_cache_bypass $http_upgrade;
        
        proxy_connect_timeout 60s;
        proxy_send_timeout 60s;
        proxy_read_timeout 60s;
    }

    # Health check
    location /health {
        proxy_pass http://127.0.0.1:8090/health;
    }
}
EOF

# Enable site
ln -sf /etc/nginx/sites-available/xfchess /etc/nginx/sites-enabled/
rm -f /etc/nginx/sites-enabled/default

nginx -t && systemctl restart nginx
log_info "Nginx configured"

# 9. Firewall Setup
log_info "Configuring firewall..."
ufw default deny incoming
ufw default allow outgoing
ufw allow 80/tcp
ufw allow 443/tcp
ufw allow 22/tcp
ufw --force enable
log_info "Firewall configured"

# 10. Systemd Service
cat > /etc/systemd/system/xfchess-backend.service << EOF
[Unit]
Description=XFChess Signing Server
After=network.target

[Service]
Type=simple
User=xfchess
Group=xfchess
WorkingDirectory=$BACKEND_DIR
Environment=RUST_LOG=info
EnvironmentFile=$BACKEND_DIR/.env
ExecStart=$BACKEND_DIR/signing-server
Restart=always
RestartSec=5
StandardOutput=journal
StandardError=journal

# Security hardening
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=$DATA_DIR
PrivateTmp=true

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable xfchess-backend
log_info "Systemd service created"

# 11. Backup Script
cat > $APP_DIR/backup.sh << 'EOF'
#!/bin/bash
BACKUP_DIR="/opt/xfchess/backups"
DATE=$(date +%Y%m%d_%H%M%S)
mkdir -p $BACKUP_DIR

# Backup databases
sqlite3 /opt/xfchess/data/sessions.db ".backup '$BACKUP_DIR/sessions_$DATE.db'"
sqlite3 /opt/xfchess/data/vault.db ".backup '$BACKUP_DIR/vault_$DATE.db'"

# Keep only last 7 days
find $BACKUP_DIR -name "*.db" -mtime +7 -delete

echo "Backup completed: $DATE"
EOF

chmod +x $APP_DIR/backup.sh
echo "0 3 * * * /opt/xfchess/backup.sh" | crontab -
log_info "Backup script installed"

# 12. Instructions
echo ""
echo "==================================="
log_info "Deployment Setup Complete!"
echo "==================================="
echo ""
echo "NEXT STEPS:"
echo "1. Copy the built frontend files to $FRONTEND_DIR"
echo "   From your local machine:"
echo "   scp -r dist/* root@178.104.55.19:$FRONTEND_DIR/"
echo ""
echo "2. Update the .env file with your actual keys:"
echo "   nano $BACKEND_DIR/.env"
echo ""
echo "   Required keys to set:"
echo "   - VPS_AUTHORITY_KEY (base58 private key)"
echo "   - KYC_AUTHORITY_KEY (base58 private key)"
echo "   - FEE_PAYER_KEYS (base58 private key)"
echo "   - SENDGRID_API_KEY (from sendgrid.com)"
echo ""
echo "3. Start the backend:"
echo "   systemctl start xfchess-backend"
echo ""
echo "4. Check status:"
echo "   systemctl status xfchess-backend"
echo "   journalctl -u xfchess-backend -f"
echo ""
echo "5. Test the deployment:"
echo "   curl http://178.104.55.19/health"
echo "   curl http://178.104.55.19/api/health"
echo ""
echo "6. Visit in browser:"
echo "   http://178.104.55.19"
echo ""
