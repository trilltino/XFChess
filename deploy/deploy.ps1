# XFChess full deploy script
# Usage: .\deploy\deploy.ps1 -Server 178.104.55.19 -User root
# Prerequisites: ssh + scp in PATH (Windows OpenSSH or Git Bash), git in PATH

param(
    [string]$Server = "178.104.55.19",
    [string]$User   = "root"
)

$SSH_KEY = "$env:USERPROFILE\.ssh\id_xfchess"
$SSH_ARGS = @('-i', $SSH_KEY, '-o', 'StrictHostKeyChecking=accept-new')
$DEST = "${User}@${Server}"
$ROOT = Split-Path $PSScriptRoot -Parent

# One-time SSH key setup
if (-not (Test-Path $SSH_KEY)) {
    Write-Host "Generating SSH key..." -ForegroundColor Yellow
    ssh-keygen -t ed25519 -f $SSH_KEY -N '""' -C xfchess-deploy
}

# Check if key is on server
$null = & ssh @SSH_ARGS -o ConnectTimeout=5 $DEST "echo key_works" 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Host "`nSSH key not on server. Type your password ONE TIME to copy it:" -ForegroundColor Yellow
    Get-Content "$SSH_KEY.pub" | & ssh -o StrictHostKeyChecking=accept-new $DEST "mkdir -p ~/.ssh && cat >> ~/.ssh/authorized_keys && chmod 600 ~/.ssh/authorized_keys"
    if ($LASTEXITCODE -ne 0) {
        Write-Host "Failed to copy key." -ForegroundColor Red
        exit 1
    }
    Write-Host "SSH key copied. Future deploys will be passwordless." -ForegroundColor Green
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
# GIT PREFLIGHT — runs before any build or upload
# ════════════════════════════════════════════════════════════════════════════════
Write-Host "`n=== Git preflight checks ===" -ForegroundColor Magenta

Push-Location $ROOT

# 1. Repo identity — must be the XFChess repo
$remoteUrl = git remote get-url origin 2>&1
if ($LASTEXITCODE -ne 0 -or $remoteUrl -notmatch "XFChess") {
    Write-Host "ABORT: This does not look like the XFChess repository." -ForegroundColor Red
    Write-Host "       Remote URL: $remoteUrl" -ForegroundColor Red
    exit 1
}
Write-Host "Repo:   $remoteUrl" -ForegroundColor Green

# 2. Show current branch — no restriction, deploy latest on any branch
$branch = git rev-parse --abbrev-ref HEAD 2>&1
Write-Host "Branch: $branch" -ForegroundColor Green

# 3. Dirty working tree — auto-commit + push
$dirty = git status --porcelain 2>&1
if ($dirty) {
    Write-Host ""
    Write-Host "  Uncommitted changes detected - auto-committing and pushing..." -ForegroundColor Yellow
    git add -A
    git commit -m "Deploy $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss')"
    git push origin $branch
    if ($LASTEXITCODE -ne 0) {
        Write-Host "  ABORT: git push failed." -ForegroundColor Red
        exit 1
    }
    Write-Host "  Committed and pushed to origin/$branch." -ForegroundColor Green
} else {
    Write-Host "Tree:   clean" -ForegroundColor Green
}

# 4. Remote sync — HARD STOP: local must not be behind origin (must be latest)
git fetch --quiet 2>&1 | Out-Null
$behind = git rev-list "HEAD..origin/$branch" --count 2>&1
if ($behind -match '^\d+$' -and [int]$behind -gt 0) {
    Write-Host ""
    Write-Host "  ABORT: Your branch is $behind commit(s) behind origin/$branch." -ForegroundColor Red
    Write-Host "  Run: git pull - then deploy again." -ForegroundColor Yellow
    exit 1
}
Write-Host "Sync:   up to date with origin/$branch" -ForegroundColor Green

# 5. Show exactly what is going out
$commitHash    = git rev-parse --short HEAD
$commitMsg     = git log -1 --pretty="%s"
$commitAuthor  = git log -1 --pretty="%an"
$commitDate    = git log -1 --pretty="%cd" --date=format:"%Y-%m-%d %H:%M"
Write-Host ""
Write-Host "  Deploying commit: $commitHash" -ForegroundColor Cyan
Write-Host "  Message:  $commitMsg"
Write-Host "  Author:   $commitAuthor"
Write-Host "  Date:     $commitDate"
Write-Host ""

Pop-Location
# ════════════════════════════════════════════════════════════════════════════════

# ── Step 1: Build frontend ────────────────────────────────────────────────────
Write-Host "`n=== Building frontend ===" -ForegroundColor Green
Push-Location "$ROOT\web-solana"
if (-not (Test-Path ".env.production")) {
    "VITE_BACKEND_URL=http://${Server}" | Out-File -Encoding utf8 ".env.production"
    Write-Host "Created .env.production with VITE_BACKEND_URL=http://${Server}" -ForegroundColor Yellow
} else {
    "VITE_BACKEND_URL=http://${Server}" | Out-File -Encoding utf8 ".env.production"
}
npm run build
if ($LASTEXITCODE -ne 0) { throw "Frontend build failed" }
Pop-Location

# ── Step 2: Server setup ──────────────────────────────────────────────────────
Write-Host "`n=== Setting up server ===" -ForegroundColor Green
Run-Remote "id xfchess 2>/dev/null || adduser xfchess --disabled-password --gecos ''"
Run-Remote "mkdir -p /opt/xfchess/data /opt/xfchess/web /opt/xfchess/backups /opt/xfchess/keys /opt/xfchess/src"
Run-Remote "chown -R xfchess:xfchess /opt/xfchess"
Run-Remote "apt-get update -qq && apt-get install -y -qq nginx sqlite3 git curl build-essential pkg-config libssl-dev ca-certificates"
Run-Remote "command -v cargo >/dev/null 2>&1 || (curl https://sh.rustup.rs -sSf | su - xfchess -c 'sh -s -- -y')"

# Install nightly cron backup (3am UTC, keeps 14 days)
$cronJob = '0 3 * * * sqlite3 /opt/xfchess/data/sessions.db ".backup ''/opt/xfchess/backups/sessions-$(date +%%Y%%m%%d).db''" ; sqlite3 /opt/xfchess/data/vault.db ".backup ''/opt/xfchess/backups/vault-$(date +%%Y%%m%%d).db''" ; find /opt/xfchess/backups -name ''*.db'' -mtime +14 -delete'
Run-Remote "(crontab -l 2>/dev/null | grep -v xfchess/backups; echo '$cronJob') | crontab -"
Write-Host "Nightly backup cron installed (3am UTC, 14-day retention)" -ForegroundColor DarkGray

# ── Step 3: Sync source and build backend on server ───────────────────────────
Write-Host "`n=== Syncing source and building backend on server ===" -ForegroundColor Green
Run-Remote "if [ ! -d /opt/xfchess/src/.git ]; then git clone --depth 1 $remoteUrl /opt/xfchess/src; fi"
Run-Remote "cd /opt/xfchess/src && git fetch --all --tags --prune && git checkout $commitHash && git reset --hard $commitHash"
Run-Remote "su - xfchess -c 'export PATH=\$HOME/.cargo/bin:\$PATH && cd /opt/xfchess/src/backend && cargo build --release --bin signing-server-http'"

# ── Step 4: Backup databases + binary before touching anything ───────────────
Write-Host "`n=== Snapshotting databases ===" -ForegroundColor Green
# Uses SQLite .backup command — safe on a live WAL-mode database (no corruption)
# Keeps the last 7 snapshots; older ones are pruned automatically
$ts = & ssh @SSH_ARGS $DEST 'date +%Y%m%d-%H%M%S'
Run-Remote "mkdir -p /opt/xfchess/backups"
Run-Remote "sqlite3 /opt/xfchess/data/sessions.db '.backup /opt/xfchess/backups/sessions-${ts}.db' 2>/dev/null || cp /opt/xfchess/data/sessions.db /opt/xfchess/backups/sessions-${ts}.db 2>/dev/null || true"
Run-Remote "sqlite3 /opt/xfchess/data/vault.db '.backup /opt/xfchess/backups/vault-${ts}.db' 2>/dev/null || cp /opt/xfchess/data/vault.db /opt/xfchess/backups/vault-${ts}.db 2>/dev/null || true"
# Prune: keep only the 7 most recent backups of each db
Run-Remote "ls -t /opt/xfchess/backups/sessions-*.db 2>/dev/null | tail -n +8 | xargs rm -f"
Run-Remote "ls -t /opt/xfchess/backups/vault-*.db    2>/dev/null | tail -n +8 | xargs rm -f"
Write-Host "DB snapshot: sessions-${ts}.db + vault-${ts}.db (7 kept)" -ForegroundColor DarkGray

Write-Host "`n=== Backing up current binary ===" -ForegroundColor Green
Run-Remote "cp /opt/xfchess/signing-server-http /opt/xfchess/signing-server-http.prev 2>/dev/null || true"
Write-Host "Binary backup saved as signing-server-http.prev" -ForegroundColor DarkGray

Write-Host "`n=== Installing Linux backend binary ===" -ForegroundColor Green
Run-Remote "cp /opt/xfchess/src/backend/target/release/signing-server-http /opt/xfchess/signing-server-http"
Run-Remote "chmod +x /opt/xfchess/signing-server-http"

# ── Step 5a: Upload keypair files ────────────────────────────────────────────
Write-Host "`n=== Uploading keypair files ===" -ForegroundColor Green
Run-Remote "mkdir -p /opt/xfchess/keys && chmod 700 /opt/xfchess/keys"
$keyFiles = @{
    "C:\Users\isich\.config\solana\id.json"           = "/opt/xfchess/keys/id.json"
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
# ── Step 5: Upload .env if it exists ─────────────────────────────────────────
Write-Host "`n=== Checking .env ===" -ForegroundColor Green
$envFile = "$ROOT\deploy\.env.production"
if (Test-Path $envFile) {
    Upload $envFile "/opt/xfchess/.env"
    Run-Remote "chmod 600 /opt/xfchess/.env"
    Write-Host ".env uploaded" -ForegroundColor Green
} else {
    Write-Host "WARNING: $envFile not found. Create it from deploy/.env.example before the server will start." -ForegroundColor Red
    Write-Host "Required minimum content:" -ForegroundColor Yellow
    Write-Host "  JWT_SECRET=<openssl rand -hex 32>"
    Write-Host "  IDENTITY_ENCRYPTION_KEY=<openssl rand -hex 32>"
    Write-Host "  IDENTITY_SALT=<openssl rand -hex 32>"
    Write-Host "  ALLOWED_ORIGINS=http://${Server}"
    Write-Host "  SESSION_DB_URL=sqlite:///opt/xfchess/data/sessions.db?mode=rwc"
    Write-Host "  VAULT_DB_URL=sqlite:///opt/xfchess/data/vault.db?mode=rwc"
} 
# ── Step 6: Upload systemd service ───────────────────────────────────────────
Write-Host "`n=== Installing systemd service ===" -ForegroundColor Green
Upload "$ROOT\deploy\xfchess-backend.service" "/etc/systemd/system/xfchess-backend.service"
Run-Remote "systemctl daemon-reload"
Run-Remote "systemctl enable xfchess-backend"
Run-Remote "systemctl restart xfchess-backend"

# ── Step 7: Upload frontend ───────────────────────────────────────────────────
Write-Host "`n=== Uploading frontend ===" -ForegroundColor Green
Upload "$ROOT\web-solana\dist\*" "/opt/xfchess/web/"

# ── Step 8: Configure nginx ───────────────────────────────────────────────────
Write-Host "`n=== Configuring nginx ===" -ForegroundColor Green
Upload "$ROOT\deploy\nginx.conf" "/etc/nginx/sites-available/xfchess"
Run-Remote "sed -i 's/YOUR_DOMAIN/${Server}/g' /etc/nginx/sites-available/xfchess"
Run-Remote "ln -sf /etc/nginx/sites-available/xfchess /etc/nginx/sites-enabled/xfchess"
Run-Remote "rm -f /etc/nginx/sites-enabled/default /etc/nginx/sites-enabled/default.conf"
Run-Remote "nginx -t && systemctl reload nginx"
Run-Remote "systemctl restart nginx"

# ── Step 9: Verify ────────────────────────────────────────────────────────────
Write-Host "`n=== Verifying deployment ===" -ForegroundColor Green
Start-Sleep -Seconds 3
$result = Invoke-RestMethod -Uri "http://${Server}/api/user/status/11111111111111111111111111111111" -ErrorAction SilentlyContinue
if ($result) {
    Write-Host "Backend responding: $($result | ConvertTo-Json)" -ForegroundColor Green
} else {
    Write-Host "Backend check failed - check logs: ssh ${DEST} journalctl -u xfchess-backend -n 50" -ForegroundColor Red
}
# Add health endpoint check
$healthResult = Invoke-RestMethod -Uri "http://${Server}/health" -ErrorAction SilentlyContinue
if ($healthResult -eq "OK") {
    Write-Host "Health endpoint check passed." -ForegroundColor Green
} else {
    Write-Host "Health endpoint check failed - check logs: ssh ${DEST} journalctl -u xfchess-backend -n 50" -ForegroundColor Red
}

Write-Host "`n=== Deploy complete ===" -ForegroundColor Green
Write-Host "Frontend: http://${Server}" -ForegroundColor Cyan
Write-Host "API:      http://${Server}/api/user/status/<wallet>" -ForegroundColor Cyan
Write-Host "Logs:     ssh ${DEST} journalctl -u xfchess-backend -f" -ForegroundColor Cyan
