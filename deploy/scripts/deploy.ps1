# XFChess full deploy script
# Usage: .\deploy\scripts\deploy.ps1 -Server 178.104.55.19 [-Domain xfchess.example.com]
#
# First run: connects as root, creates deploy user, hardens server, obtains SSL cert.
# Subsequent runs: connects as deploy (or root if -User root is passed explicitly).
#
# Prerequisites: ssh + scp in PATH (Windows OpenSSH or Git Bash), git in PATH

param(
    [string]$Server  = "178.104.55.19",
    [string]$User    = "root",
    [string]$Domain  = "",   # Real domain for Let's Encrypt (leave blank to use self-signed)
    [switch]$SkipBuild       # Skip frontend/backend build (re-deploy existing binaries)
)

$SSH_KEY  = "$env:USERPROFILE\.ssh\id_xfchess"
$SSH_ARGS = @('-i', $SSH_KEY, '-o', 'StrictHostKeyChecking=accept-new')
$DEST     = "${User}@${Server}"
$ROOT     = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent   # repo root
$TlsDomain = if ($Domain) { $Domain } else { $Server }

# ── SSH key bootstrap ─────────────────────────────────────────────────────────
if (-not (Test-Path $SSH_KEY)) {
    Write-Host "Generating SSH key..." -ForegroundColor Yellow
    ssh-keygen -t ed25519 -f $SSH_KEY -N '""' -C xfchess-deploy
    Write-Host "SSH key generated at $SSH_KEY" -ForegroundColor Green
} else {
    $testKey = & ssh-keygen -y -P "" -f $SSH_KEY 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-Host "Existing key has passphrase; regenerating without passphrase for automation." -ForegroundColor Yellow
        Remove-Item $SSH_KEY
        Remove-Item "$SSH_KEY.pub"
        ssh-keygen -t ed25519 -f $SSH_KEY -N '""' -C xfchess-deploy
    }
}

Write-Host "`nCopying SSH key to server..." -ForegroundColor Yellow
$authKeysCmd = 'mkdir -p ~/.ssh && cat >> ~/.ssh/authorized_keys && chmod 600 ~/.ssh/authorized_keys'
Get-Content "$SSH_KEY.pub" | & ssh -o StrictHostKeyChecking=accept-new $DEST $authKeysCmd
if ($LASTEXITCODE -ne 0) {
    Write-Host "Failed to copy key — you may need to enter the password once." -ForegroundColor Yellow
} else {
    Write-Host "SSH key copied." -ForegroundColor Green
}

function Test-SSHKey {
    $null = & ssh @SSH_ARGS -o ConnectTimeout=5 $DEST "echo key_works" 2>&1
    return $LASTEXITCODE -eq 0
}
if (-not (Test-SSHKey)) {
    Get-Content "$SSH_KEY.pub" | & ssh -o StrictHostKeyChecking=accept-new $DEST $authKeysCmd
    if ($LASTEXITCODE -ne 0) { Write-Host "SSH auth failed." -ForegroundColor Red; exit 1 }
}

function Run-Remote($cmd) {
    Write-Host ">> $cmd" -ForegroundColor Cyan
    & ssh @SSH_ARGS $DEST $cmd
    if ($LASTEXITCODE -ne 0) { throw "Remote command failed: $cmd" }
}

function Upload($local, $remote) {
    Write-Host ">> scp $local -> $remote" -ForegroundColor Yellow
    & scp @SSH_ARGS -r $local "${DEST}:${remote}"
    if ($LASTEXITCODE -ne 0) { throw "Upload failed: $local" }
}

# ════════════════════════════════════════════════════════════════════════════════
# GIT PREFLIGHT
# ════════════════════════════════════════════════════════════════════════════════
Write-Host "`n=== Git preflight checks ===" -ForegroundColor Magenta
Push-Location $ROOT

$remoteUrl = git remote get-url origin 2>&1
if ($LASTEXITCODE -ne 0 -or $remoteUrl -notmatch "XFChess") {
    Write-Host "ABORT: This does not look like the XFChess repository." -ForegroundColor Red; exit 1
}
Write-Host "Repo:   $remoteUrl" -ForegroundColor Green

$branch = git rev-parse --abbrev-ref HEAD 2>&1
Write-Host "Branch: $branch" -ForegroundColor Green

$dirty = git status --porcelain 2>&1
if ($dirty) {
    Write-Host "Uncommitted changes detected — auto-committing..." -ForegroundColor Yellow
    git add -A
    git commit -m "Deploy $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss')"
    git push origin $branch
    if ($LASTEXITCODE -ne 0) { Write-Host "ABORT: git push failed." -ForegroundColor Red; exit 1 }
}

git fetch --quiet 2>&1 | Out-Null
$behind = git rev-list "HEAD..origin/$branch" --count 2>&1
if ($behind -match '^\d+$' -and [int]$behind -gt 0) {
    Write-Host "ABORT: $behind commit(s) behind origin/$branch. Run: git pull" -ForegroundColor Red; exit 1
}
Write-Host "Sync:   up to date with origin/$branch" -ForegroundColor Green

$commitHash   = git rev-parse --short HEAD
$commitMsg    = git log -1 --pretty="%s"
$commitAuthor = git log -1 --pretty="%an"
$commitDate   = git log -1 --pretty="%cd" --date=format:"%Y-%m-%d %H:%M"
Write-Host "`n  Deploying: $commitHash — $commitMsg ($commitAuthor, $commitDate)`n" -ForegroundColor Cyan
Pop-Location

# ── Step 1: Build frontend ────────────────────────────────────────────────────
if (-not $SkipBuild) {
    Write-Host "`n=== Building frontend ===" -ForegroundColor Green
    Push-Location "$ROOT\web-solana"
    "VITE_BACKEND_URL=https://${TlsDomain}`nSIGNING_SERVICE_URL=https://${TlsDomain}" | Out-File -Encoding utf8 ".env.production"
    Write-Host "VITE_BACKEND_URL=https://${TlsDomain}" -ForegroundColor DarkGray
    npm run build
    if ($LASTEXITCODE -ne 0) { throw "Frontend build failed" }
    Pop-Location
}

# ── Step 2: Server base setup ─────────────────────────────────────────────────
Write-Host "`n=== Setting up server ===" -ForegroundColor Green
Run-Remote "id xfchess 2>/dev/null || adduser xfchess --disabled-password --gecos ''"
Run-Remote "mkdir -p /home/xfchess && chown xfchess:xfchess /home/xfchess"
Run-Remote "mkdir -p /opt/xfchess/data /opt/xfchess/web /opt/xfchess/backups /opt/xfchess/keys /opt/xfchess/src"
Run-Remote "chown -R xfchess:xfchess /opt/xfchess"

Run-Remote "apt-get update -qq && apt-get install -y -qq nginx sqlite3 git curl build-essential pkg-config libssl-dev ca-certificates certbot python3-certbot-nginx ufw logrotate"
Run-Remote "command -v cargo >/dev/null 2>&1 || (curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs -o /tmp/rustup.sh && chmod +x /tmp/rustup.sh && /tmp/rustup.sh -y)"

# ── Step 2a: Deploy user with restricted sudo ─────────────────────────────────
Write-Host "`n=== Creating deploy user ===" -ForegroundColor Green
Run-Remote "id deploy 2>/dev/null || adduser deploy --disabled-password --gecos ''"
# Restricted sudo: only allowed to restart the backend and reload nginx — no shell escalation
Run-Remote @"
cat > /etc/sudoers.d/deploy-xfchess << 'SUDOEOF'
deploy ALL=(root) NOPASSWD: /bin/systemctl restart xfchess-backend, /bin/systemctl reload nginx, /bin/systemctl status xfchess-backend
SUDOEOF
chmod 440 /etc/sudoers.d/deploy-xfchess
"@
# Copy the deploy key to the deploy user too
Run-Remote "mkdir -p /home/deploy/.ssh && chmod 700 /home/deploy/.ssh && chown deploy:deploy /home/deploy/.ssh"
Run-Remote "cat /root/.ssh/authorized_keys >> /home/deploy/.ssh/authorized_keys 2>/dev/null || true"
Run-Remote "chown deploy:deploy /home/deploy/.ssh/authorized_keys && chmod 600 /home/deploy/.ssh/authorized_keys"
Write-Host "deploy user created. After this run, use -User deploy for future deploys." -ForegroundColor DarkGray

# ── Step 2b: SSH hardening (disable password auth, prohibit root password login) ──
Write-Host "`n=== Hardening SSH ===" -ForegroundColor Green
Run-Remote "sed -i 's/^#\?PermitRootLogin.*/PermitRootLogin prohibit-password/' /etc/ssh/sshd_config"
Run-Remote "sed -i 's/^#\?PasswordAuthentication.*/PasswordAuthentication no/' /etc/ssh/sshd_config"
Run-Remote "sshd -t && systemctl reload sshd"
Write-Host "Root password login disabled; key-based root access retained for this deploy." -ForegroundColor DarkGray

# ── Step 2c: UFW firewall ─────────────────────────────────────────────────────
Write-Host "`n=== Configuring UFW firewall ===" -ForegroundColor Green
Run-Remote "ufw --force reset"
Run-Remote "ufw default deny incoming"
Run-Remote "ufw default allow outgoing"
Run-Remote "ufw allow 22/tcp comment 'SSH'"
Run-Remote "ufw allow 80/tcp comment 'HTTP (redirect to HTTPS)'"
Run-Remote "ufw allow 443/tcp comment 'HTTPS'"
# Prometheus and node_exporter are only accessible from localhost — no public rule
# Backend port 8090 is intentionally not exposed; nginx proxies it
Run-Remote "ufw --force enable"
Run-Remote "ufw status verbose"
Write-Host "UFW enabled: 22/80/443 open; 8090/9090/9100 internal only." -ForegroundColor DarkGray

# ── Step 2d: node_exporter ────────────────────────────────────────────────────
Write-Host "`n=== Installing node_exporter ===" -ForegroundColor Green
Run-Remote @"
if ! command -v node_exporter >/dev/null 2>&1; then
    NE_VER=1.8.2
    curl -fsSL https://github.com/prometheus/node_exporter/releases/download/v\${NE_VER}/node_exporter-\${NE_VER}.linux-amd64.tar.gz | tar xz -C /tmp
    mv /tmp/node_exporter-\${NE_VER}.linux-amd64/node_exporter /usr/local/bin/node_exporter
    chmod +x /usr/local/bin/node_exporter
fi
"@
Run-Remote @"
id node_exporter 2>/dev/null || useradd --no-create-home --shell /bin/false node_exporter
cat > /etc/systemd/system/node_exporter.service << 'NEEOF'
[Unit]
Description=Prometheus Node Exporter
After=network.target

[Service]
User=node_exporter
ExecStart=/usr/local/bin/node_exporter --web.listen-address=127.0.0.1:9100
Restart=on-failure
RestartSec=5
NoNewPrivileges=true
ProtectSystem=strict
PrivateTmp=true

[Install]
WantedBy=multi-user.target
NEEOF
systemctl daemon-reload && systemctl enable node_exporter && systemctl restart node_exporter
"@
Write-Host "node_exporter installed and running on 127.0.0.1:9100." -ForegroundColor DarkGray

# ── Step 2e: Log rotation ─────────────────────────────────────────────────────
Write-Host "`n=== Configuring log rotation ===" -ForegroundColor Green
Run-Remote @"
cat > /etc/logrotate.d/xfchess << 'LREOF'
/var/log/xfchess/*.log {
    daily
    rotate 14
    compress
    delaycompress
    missingok
    notifempty
    create 0640 xfchess xfchess
    postrotate
        systemctl reload xfchess-backend 2>/dev/null || true
    endscript
}
LREOF
"@
# Cap journald at 1 GB to prevent disk exhaustion from noisy logs
Run-Remote "mkdir -p /var/log/xfchess"
Run-Remote "sed -i 's/^#\?SystemMaxUse=.*/SystemMaxUse=1G/' /etc/systemd/journald.conf"
Run-Remote "sed -i 's/^#\?RuntimeMaxUse=.*/RuntimeMaxUse=256M/' /etc/systemd/journald.conf"
Run-Remote "systemctl restart systemd-journald"
Write-Host "logrotate configured (14-day, daily); journald capped at 1 GB." -ForegroundColor DarkGray

# ── Step 2f: Offsite backup (rclone → Backblaze B2) ──────────────────────────
Write-Host "`n=== Setting up offsite backup ===" -ForegroundColor Green
Run-Remote @"
if ! command -v rclone >/dev/null 2>&1; then
    curl -fsSL https://rclone.org/install.sh | bash -s -- --install-only
fi
"@
# The rclone remote 'b2xfchess' must be configured manually once:
#   ssh root@SERVER rclone config
#   Choose: New remote -> name: b2xfchess -> type: b2 -> enter account/key
# Daily 3am: SQLite .backup (online-safe) + rclone sync to B2, 7-day local retention
Run-Remote @"
(crontab -l 2>/dev/null | grep -v xfchess/backups; echo '0 3 * * * STAMP=\$(date +%%Y%%m%%d-%%H%%M%%S); sqlite3 /opt/xfchess/data/sessions.db ".backup /opt/xfchess/backups/sessions-\${STAMP}.db" 2>/dev/null; sqlite3 /opt/xfchess/data/vault.db ".backup /opt/xfchess/backups/vault-\${STAMP}.db" 2>/dev/null; rclone sync /opt/xfchess/backups b2xfchess:xfchess-backups --min-age 1s 2>/dev/null || true; find /opt/xfchess/backups -name "*.db" -mtime +7 -delete') | crontab -
"@
Write-Host "Backup cron: 3am UTC daily, rclone to b2xfchess:xfchess-backups, 7-day local retention." -ForegroundColor DarkGray
Write-Host "ACTION REQUIRED: run 'rclone config' on the server to set up the B2 remote." -ForegroundColor Yellow

# ── Step 3: Sync source and build backend ─────────────────────────────────────
if (-not $SkipBuild) {
    Write-Host "`n=== Syncing source and building backend on server ===" -ForegroundColor Green
    Run-Remote "if [ ! -d /opt/xfchess/src/.git ]; then git clone --depth 1 $remoteUrl /opt/xfchess/src; fi"
    Run-Remote "cd /opt/xfchess/src && git fetch --all --tags --prune && git checkout $commitHash && git reset --hard $commitHash"
    Run-Remote "su - xfchess -c 'export PATH=\$HOME/.cargo/bin:\$PATH && cd /opt/xfchess/src/backend && cargo build --release --bin signing-server-http'"
}

# ── Step 4: Snapshot databases + binary ──────────────────────────────────────
Write-Host "`n=== Snapshotting databases ===" -ForegroundColor Green
$ts = & ssh @SSH_ARGS $DEST 'date +%Y%m%d-%H%M%S'
Run-Remote "mkdir -p /opt/xfchess/backups"
Run-Remote "sqlite3 /opt/xfchess/data/sessions.db '.backup /opt/xfchess/backups/sessions-${ts}.db' 2>/dev/null || cp /opt/xfchess/data/sessions.db /opt/xfchess/backups/sessions-${ts}.db 2>/dev/null || true"
Run-Remote "sqlite3 /opt/xfchess/data/vault.db    '.backup /opt/xfchess/backups/vault-${ts}.db'    2>/dev/null || cp /opt/xfchess/data/vault.db    /opt/xfchess/backups/vault-${ts}.db    2>/dev/null || true"
Run-Remote "ls -t /opt/xfchess/backups/sessions-*.db 2>/dev/null | tail -n +8 | xargs rm -f"
Run-Remote "ls -t /opt/xfchess/backups/vault-*.db    2>/dev/null | tail -n +8 | xargs rm -f"
Write-Host "DB snapshot: sessions-${ts}.db + vault-${ts}.db (7 kept)" -ForegroundColor DarkGray

Write-Host "`n=== Backing up current binary ===" -ForegroundColor Green
Run-Remote "cp /opt/xfchess/signing-server-http /opt/xfchess/signing-server-http.prev 2>/dev/null || true"

if (-not $SkipBuild) {
    Write-Host "`n=== Installing Linux backend binary ===" -ForegroundColor Green
    Run-Remote "cp /opt/xfchess/src/backend/target/release/signing-server-http /opt/xfchess/signing-server-http"
    Run-Remote "chmod +x /opt/xfchess/signing-server-http"
}

# ── Step 5a: Upload keypair files ─────────────────────────────────────────────
Write-Host "`n=== Uploading keypair files ===" -ForegroundColor Green
Run-Remote "mkdir -p /opt/xfchess/keys && chmod 700 /opt/xfchess/keys"
$keyFiles = @{
    "C:\Users\isich\.config\solana\id.json"              = "/opt/xfchess/keys/id.json"
    "C:\Users\isich\.config\xfchess\relayer-devnet.json" = "/opt/xfchess/keys/relayer-devnet.json"
}
foreach ($src in $keyFiles.Keys) {
    if (Test-Path $src) {
        Upload $src $keyFiles[$src]
        Run-Remote "chmod 600 $($keyFiles[$src])"
        Write-Host "Uploaded $([System.IO.Path]::GetFileName($src))" -ForegroundColor Green
    } else {
        Write-Host "WARNING: $src not found - skipping" -ForegroundColor Red
    }
}

# ── Step 5b: Upload .env ──────────────────────────────────────────────────────
Write-Host "`n=== Checking .env ===" -ForegroundColor Green
$envFile = "$ROOT\deploy\.env.production"
if (Test-Path $envFile) {
    Upload $envFile "/opt/xfchess/.env"
    Run-Remote "chmod 600 /opt/xfchess/.env"
    Write-Host ".env uploaded" -ForegroundColor Green
} else {
    Write-Host "WARNING: $envFile not found. Required minimum content:" -ForegroundColor Red
    Write-Host "  JWT_SECRET=<openssl rand -hex 32>"
    Write-Host "  IDENTITY_ENCRYPTION_KEY=<openssl rand -hex 32>"
    Write-Host "  IDENTITY_SALT=<openssl rand -hex 32>"
    Write-Host "  ALLOWED_ORIGINS=https://${TlsDomain}"
    Write-Host "  SESSION_DB_URL=sqlite:///opt/xfchess/data/sessions.db?mode=rwc"
    Write-Host "  VAULT_DB_URL=sqlite:///opt/xfchess/data/vault.db?mode=rwc"
}

# ── Step 6: Install systemd service ──────────────────────────────────────────
Write-Host "`n=== Installing systemd service ===" -ForegroundColor Green
Upload "$ROOT\deploy\backend\xfchess-backend.service" "/etc/systemd/system/xfchess-backend.service"
Run-Remote "systemctl daemon-reload"
Run-Remote "systemctl enable xfchess-backend"
Run-Remote "systemctl restart xfchess-backend"

# ── Step 7: Upload frontend ───────────────────────────────────────────────────
Write-Host "`n=== Uploading frontend ===" -ForegroundColor Green
Upload "$ROOT\web-solana\dist\*" "/opt/xfchess/web/"

# ── Step 8: Configure nginx ───────────────────────────────────────────────────
Write-Host "`n=== Configuring nginx ===" -ForegroundColor Green
Run-Remote "mkdir -p /etc/nginx/conf.d"
Upload "$ROOT\deploy\nginx\xfchess_rate_limit.conf" "/etc/nginx/conf.d/xfchess_rate_limit.conf"
Upload "$ROOT\deploy\nginx\nginx.conf" "/etc/nginx/sites-available/xfchess"
Run-Remote "sed -i 's/YOUR_DOMAIN/${TlsDomain}/g' /etc/nginx/sites-available/xfchess"
Run-Remote "ln -sf /etc/nginx/sites-available/xfchess /etc/nginx/sites-enabled/xfchess"
Run-Remote "rm -f /etc/nginx/sites-enabled/default /etc/nginx/sites-enabled/default.conf"
Run-Remote "mkdir -p /var/www/certbot"

# SSL certificate: prefer Let's Encrypt (real domain), fall back to self-signed (IP)
if ($Domain) {
    Write-Host "`n=== Obtaining Let's Encrypt certificate for $Domain ===" -ForegroundColor Green
    # First bring nginx up in HTTP-only mode so ACME challenge works
    Run-Remote @"
nginx -t 2>/dev/null || true
# Temporarily serve HTTP for certbot if HTTPS certs don't exist yet
test -f /etc/letsencrypt/live/${Domain}/fullchain.pem || \
  certbot certonly --webroot -w /var/www/certbot -d ${Domain} --non-interactive --agree-tos -m admin@${Domain} --quiet
"@
    Run-Remote "nginx -t && systemctl reload nginx && systemctl restart nginx"
    # Auto-renew hook
    Run-Remote "(crontab -l 2>/dev/null | grep -v certbot; echo '0 2 * * 1 certbot renew --quiet --post-hook \"systemctl reload nginx\"') | crontab -"
    Write-Host "Certbot renewal cron: every Monday 2am UTC." -ForegroundColor DarkGray
} else {
    Write-Host "`n=== Generating self-signed TLS certificate (no domain provided) ===" -ForegroundColor Yellow
    Run-Remote @"
mkdir -p /etc/letsencrypt/live/${Server}
test -f /etc/letsencrypt/live/${Server}/fullchain.pem || \
  openssl req -x509 -nodes -days 365 -newkey rsa:2048 \
    -keyout /etc/letsencrypt/live/${Server}/privkey.pem \
    -out    /etc/letsencrypt/live/${Server}/fullchain.pem \
    -subj '/CN=${Server}/O=XFChess/C=US' \
    -extensions v3_ca \
    -addext 'subjectAltName=IP:${Server}'
"@
    Run-Remote "nginx -t && systemctl reload nginx && systemctl restart nginx"
    Write-Host "Self-signed cert installed. For production, pass -Domain your.domain.com and rerun." -ForegroundColor Yellow
}

# ── Step 9: Upload Prometheus monitoring config ───────────────────────────────
Write-Host "`n=== Updating Prometheus config ===" -ForegroundColor Green
if (& ssh @SSH_ARGS $DEST "test -f /etc/prometheus/prometheus.yml && echo yes" 2>$null) {
    Upload "$ROOT\deploy\monitoring\prometheus.yml" "/etc/prometheus/prometheus.yml"
    Run-Remote "mkdir -p /etc/prometheus/rules"
    Upload "$ROOT\deploy\monitoring\rules\disk_alerts.yml" "/etc/prometheus/rules/disk_alerts.yml"
    Run-Remote "systemctl reload prometheus 2>/dev/null || true"
    Write-Host "Prometheus config updated; disk alert rules installed." -ForegroundColor DarkGray
} else {
    Write-Host "Prometheus not installed on this server; skipping config upload." -ForegroundColor DarkGray
}

# ── Step 10: Verify ───────────────────────────────────────────────────────────
Write-Host "`n=== Verifying deployment ===" -ForegroundColor Green
Start-Sleep -Seconds 3
$proto = "https"
$result = Invoke-RestMethod -Uri "${proto}://${TlsDomain}/api/user/status/11111111111111111111111111111111" -SkipCertificateCheck -ErrorAction SilentlyContinue
if ($result) {
    Write-Host "Backend responding: $($result | ConvertTo-Json -Compress)" -ForegroundColor Green
} else {
    Write-Host "Backend check failed — check logs: ssh ${DEST} journalctl -u xfchess-backend -n 50" -ForegroundColor Red
}
$health = Invoke-RestMethod -Uri "${proto}://${TlsDomain}/health" -SkipCertificateCheck -ErrorAction SilentlyContinue
if ($health -eq "OK") {
    Write-Host "Health endpoint OK." -ForegroundColor Green
} else {
    Write-Host "Health endpoint failed — check nginx: ssh ${DEST} nginx -t" -ForegroundColor Red
}

Write-Host "`n=== Deploy complete ===" -ForegroundColor Green
Write-Host "Frontend: https://${TlsDomain}" -ForegroundColor Cyan
Write-Host "API:      https://${TlsDomain}/api/user/status/<wallet>" -ForegroundColor Cyan
Write-Host "Logs:     ssh ${DEST} journalctl -u xfchess-backend -f" -ForegroundColor Cyan
Write-Host ""
Write-Host "Hetzner snapshot: Hetzner Cloud Console -> Servers -> ${Server} -> Snapshots -> Create snapshot" -ForegroundColor DarkGray
Write-Host "                  (Schedule weekly via Hetzner API: POST /servers/{id}/actions/create_image)" -ForegroundColor DarkGray
Write-Host "B2 backup:        Run 'rclone config' on the server once to configure b2xfchess remote." -ForegroundColor DarkGray
Write-Host "Secrets rotation: See deploy/SECRETS_ROTATION.md" -ForegroundColor DarkGray
