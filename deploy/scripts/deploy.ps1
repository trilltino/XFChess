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

$SSH_KEY  = "$env:USERPROFILE\.ssh\xfchess_vps"
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
    # Windows' bundled ssh-keygen.exe (System32\OpenSSH) rejects `-y -P ""` together
    # ("Too many arguments") regardless of whether the key has a passphrase — that
    # false positive was wiping a perfectly good key on every run. Piping empty
    # stdin instead: a real passphrase prompt reads EOF and fails fast; a
    # passphrase-less key just succeeds, ignoring stdin entirely.
    $testKey = "" | & ssh-keygen -y -f $SSH_KEY 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-Host "Existing key has passphrase; regenerating without passphrase for automation." -ForegroundColor Yellow
        Remove-Item $SSH_KEY
        Remove-Item "$SSH_KEY.pub"
        ssh-keygen -t ed25519 -f $SSH_KEY -N '""' -C xfchess-deploy
    }
}

Write-Host "`nCopying SSH key to server..." -ForegroundColor Yellow
$authKeysCmd = 'mkdir -p ~/.ssh && cat >> ~/.ssh/authorized_keys && chmod 600 ~/.ssh/authorized_keys'
# Try the bootstrapped key itself first (BatchMode=yes fails fast, no hang) — if it's
# already trusted (e.g. installed out-of-band), this succeeds instantly and we skip
# the password prompt entirely. Only a truly untrusted key falls through to the
# interactive password bootstrap below (needs a real TTY — not safe to background).
$null = & ssh @SSH_ARGS -o BatchMode=yes -o ConnectTimeout=5 $DEST "echo key_works" 2>&1
if ($LASTEXITCODE -eq 0) {
    Write-Host "Key already trusted — skipping password bootstrap." -ForegroundColor Green
} else {
    Get-Content "$SSH_KEY.pub" | & ssh -o StrictHostKeyChecking=accept-new $DEST $authKeysCmd
    if ($LASTEXITCODE -ne 0) {
        Write-Host "Failed to copy key — you may need to enter the password once." -ForegroundColor Yellow
    } else {
        Write-Host "SSH key copied." -ForegroundColor Green
    }
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
    Write-Host "ABORT: uncommitted changes in the working tree." -ForegroundColor Red
    Write-Host "Review, then commit or stash before deploying (never auto-commit secrets/junk):" -ForegroundColor Yellow
    Write-Host $dirty
    exit 1
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
    # Write BOM-less UTF-8: PS5.1 Out-File -Encoding utf8 adds a BOM, which breaks
    # env parsing (this is the class of bug that took the backend .env down).
    $frontendEnv = "VITE_BACKEND_URL=https://${TlsDomain}`nSIGNING_SERVICE_URL=https://${TlsDomain}`n"
    [System.IO.File]::WriteAllText("$ROOT\web-solana\.env.production", $frontendEnv, (New-Object System.Text.UTF8Encoding($false)))
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
# Install rustup FOR the xfchess build user (shell overridden — xfchess is nologin),
# with CARGO_HOME/RUSTUP_HOME under /opt/xfchess so the build in Step 3 can find cargo.
$rustupCmd = 'su -s /bin/bash xfchess -c ''export CARGO_HOME=/opt/xfchess/.cargo RUSTUP_HOME=/opt/xfchess/.rustup HOME=/opt/xfchess; [ -x $CARGO_HOME/bin/cargo ] || (curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs -o /tmp/rustup.sh && sh /tmp/rustup.sh -y --no-modify-path --default-toolchain stable)'''
Run-Remote $rustupCmd

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

# ── Step 2a2: Locked-down SSH tunnel user (admin panel PRODUCTION mode) ────────
# The tournament-admin panel reaches the never-public /admin/* API by forwarding
# a local port to the backend's loopback (ssh -N -L 8091:127.0.0.1:8090). This
# user is nologin and — via the sshd drop-in below — may ONLY forward to the
# backend port. A leak of its key yields a port-forward, not a shell.
Write-Host "`n=== Creating SSH tunnel user ===" -ForegroundColor Green
Run-Remote "id tunnel 2>/dev/null || adduser tunnel --disabled-password --shell /usr/sbin/nologin --gecos ''"
Run-Remote "mkdir -p /home/tunnel/.ssh && chmod 700 /home/tunnel/.ssh && chown tunnel:tunnel /home/tunnel/.ssh"
Run-Remote "cat /root/.ssh/authorized_keys >> /home/tunnel/.ssh/authorized_keys 2>/dev/null || true"
Run-Remote "sort -u /home/tunnel/.ssh/authorized_keys -o /home/tunnel/.ssh/authorized_keys"
Run-Remote "chown tunnel:tunnel /home/tunnel/.ssh/authorized_keys && chmod 600 /home/tunnel/.ssh/authorized_keys"
# Append the forward-only policy to the END of sshd_config. It must be the LAST
# block: a Match captures everything after it, and Ubuntu's `Include` sits ABOVE
# the global directives, so a drop-in Match would wrongly scope those globals.
# No ForceCommand — a forced command that exits would drop the -N port-forward;
# the nologin shell already blocks interactive/command sessions. Idempotent via marker.
Run-Remote @"
if ! grep -q 'XFCHESS-TUNNEL-MATCH' /etc/ssh/sshd_config; then
cat >> /etc/ssh/sshd_config << 'TUNEOF'

# XFCHESS-TUNNEL-MATCH (managed by deploy.ps1) — forward-only admin tunnel user.
# Must remain the LAST block in this file.
Match User tunnel
    AllowTcpForwarding yes
    PermitOpen 127.0.0.1:8090
    X11Forwarding no
    AllowAgentForwarding no
    PermitTTY no
    GatewayPorts no
TUNEOF
fi
"@
Write-Host "tunnel user created (nologin; may only forward to 127.0.0.1:8090)." -ForegroundColor DarkGray

# ── Step 2b: SSH hardening (disable password auth, prohibit root password login) ──
Write-Host "`n=== Hardening SSH ===" -ForegroundColor Green
Run-Remote "sed -i 's/^#\?PermitRootLogin.*/PermitRootLogin prohibit-password/' /etc/ssh/sshd_config"
Run-Remote "sed -i 's/^#\?PasswordAuthentication.*/PasswordAuthentication no/' /etc/ssh/sshd_config"
Run-Remote "sshd -t && (systemctl reload ssh 2>/dev/null || systemctl reload sshd)"
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
# Single-quoted here-string: PowerShell must NOT expand ${NE_VER} — it's a remote
# shell variable. (In a double-quoted here-string PS5.1 expands ${...}/$(...) even
# behind a backslash, which mangled this URL and the backup cron below.)
Run-Remote @'
if ! command -v node_exporter >/dev/null 2>&1; then
    NE_VER=1.8.2
    curl -fsSL https://github.com/prometheus/node_exporter/releases/download/v${NE_VER}/node_exporter-${NE_VER}.linux-amd64.tar.gz | tar xz -C /tmp
    mv /tmp/node_exporter-${NE_VER}.linux-amd64/node_exporter /usr/local/bin/node_exporter
    chmod +x /usr/local/bin/node_exporter
fi
'@
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
Run-Remote "command -v rclone >/dev/null 2>&1 || (apt-get update -qq && apt-get install -y -qq rclone)"
# The rclone remote 'b2xfchess' must be configured manually once:
#   ssh root@SERVER rclone config
#   Choose: New remote -> name: b2xfchess -> type: b2 -> enter account/key
# Daily 3am: SQLite .backup (online-safe) + non-SQLite data (signup/waitlist JSONL,
# game archive) + rclone sync to B2, 7-day local retention.
# Installed as a script rather than a crontab one-liner: a one-liner needs cron '%'
# escaping AND survives PS5.1 here-string expansion of $()/${} — the previous
# one-liner was silently mangled by exactly that (STAMP expanded to nothing).
# NOTE on \" below: PS5.1 passes args to native exes (ssh) without escaping — bare
# embedded " chars are eaten by the Windows argv parser and the command arrives
# quote-less. Writing \" makes a literal " arrive on the server. Single-quoted
# shell strings pass through untouched, so prefer ' where no expansion is needed.
Run-Remote @'
cat > /opt/xfchess/backup.sh << 'EOF'
#!/bin/sh
# Nightly XFChess backup. DBs are snapshotted online-safe via sqlite3 .backup;
# everything non-SQLite under data/ (subscribers.jsonl, waitlist.jsonl, archive/)
# is rclone-copied as-is. Requires the b2xfchess rclone remote (rclone config).
STAMP=$(date +%Y%m%d-%H%M%S)
sqlite3 /opt/xfchess/data/sessions.db \".backup /opt/xfchess/backups/sessions-${STAMP}.db\" 2>/dev/null
sqlite3 /opt/xfchess/data/vault.db \".backup /opt/xfchess/backups/vault-${STAMP}.db\" 2>/dev/null
rclone sync /opt/xfchess/backups b2xfchess:xfchess-backups/db --min-age 1s 2>/dev/null || true
rclone copy /opt/xfchess/data b2xfchess:xfchess-backups/data --exclude '*.db' --exclude '*.db-*' 2>/dev/null || true
find /opt/xfchess/backups -name '*.db' -mtime +7 -delete
EOF
chmod +x /opt/xfchess/backup.sh
(crontab -l 2>/dev/null | grep -v 'xfchess/backup'; echo '0 3 * * * /opt/xfchess/backup.sh') | crontab -
'@
Write-Host "Backup: 3am UTC daily via /opt/xfchess/backup.sh -> b2xfchess:xfchess-backups (db snapshots + JSONL + archive), 7-day local retention." -ForegroundColor DarkGray
Write-Host "ACTION REQUIRED: run 'rclone config' on the server to set up the B2 remote." -ForegroundColor Yellow

# ── Step 3: Sync source and build backend ─────────────────────────────────────
if (-not $SkipBuild) {
    Write-Host "`n=== Syncing source and building backend on server ===" -ForegroundColor Green
    # Clone into a clean dir: `git clone` fails if /opt/xfchess/src exists as a non-git
    # dir (older layouts put the game src there), so remove a non-git dir first.
    # A prior run's final chown leaves this owned by xfchess; root running git here
    # otherwise trips git's dubious-ownership guard (CVE-2022-24765 protection).
    Run-Remote "git config --global --add safe.directory /opt/xfchess/src"
    Run-Remote "if [ ! -d /opt/xfchess/src/.git ]; then rm -rf /opt/xfchess/src && git clone $remoteUrl /opt/xfchess/src; fi"
    # -f: a prior server-side `cargo build` regenerates Cargo.lock in the worktree,
    # which blocks a plain checkout ("local changes would be overwritten"). This
    # checkout is disposable build state, not anyone's work — safe to force past.
    Run-Remote "cd /opt/xfchess/src && git fetch --all --tags --prune && git checkout -f $commitHash && git reset --hard $commitHash && chown -R xfchess:xfchess /opt/xfchess/src"
    # Build as the (nologin) xfchess user via -s /bin/bash, with cargo on PATH; this is a
    # workspace, so build with -p backend from the repo root.
    $buildCmd = 'su -s /bin/bash xfchess -c ''export CARGO_HOME=/opt/xfchess/.cargo RUSTUP_HOME=/opt/xfchess/.rustup HOME=/opt/xfchess PATH=/opt/xfchess/.cargo/bin:$PATH && cd /opt/xfchess/src && cargo build --release -p backend --bin signing-server-http'''
    Run-Remote $buildCmd
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
    # Workspace target dir is at the repo root, not backend/target.
    # Copy to a temp name + atomic rename: the target is the currently-running
    # service's executable, and a plain in-place `cp` fails with "Text file busy"
    # (Linux won't let you write into a busy inode). `mv` replaces the directory
    # entry instead, which the kernel allows — the old process keeps running on
    # its now-unlinked inode until `systemctl restart` below picks up the new one.
    Run-Remote "cp /opt/xfchess/src/target/release/signing-server-http /opt/xfchess/signing-server-http.new && mv /opt/xfchess/signing-server-http.new /opt/xfchess/signing-server-http"
    Run-Remote "chmod +x /opt/xfchess/signing-server-http && chown xfchess:xfchess /opt/xfchess/signing-server-http"
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

# ── Step 5b: Upload .env (only if the server has none — never clobber prod secrets) ──
Write-Host "`n=== Checking .env ===" -ForegroundColor Green
$envFile = "$ROOT\deploy\.env.production"
$serverHasEnv = (& ssh @SSH_ARGS $DEST "test -f /opt/xfchess/.env && echo yes" 2>$null) -eq "yes"
if ($serverHasEnv) {
    Write-Host "Server already has /opt/xfchess/.env — preserving it (secrets kept as-is)." -ForegroundColor Green
    Write-Host "  To change config, edit it on the server: ssh ${DEST} nano /opt/xfchess/.env" -ForegroundColor DarkGray
} elseif (Test-Path $envFile) {
    Upload $envFile "/opt/xfchess/.env"
    Run-Remote "chmod 600 /opt/xfchess/.env"
    Write-Host ".env uploaded (first-time bootstrap)" -ForegroundColor Green
} else {
    Write-Host "WARNING: $envFile not found. Required minimum content:" -ForegroundColor Red
    Write-Host "  JWT_SECRET=<openssl rand -hex 32>"
    Write-Host "  IDENTITY_ENCRYPTION_KEY=<openssl rand -hex 32>"
    Write-Host "  IDENTITY_SALT=<openssl rand -hex 32>"
    # Include the tournament-admin panel's Tauri origins so PRODUCTION mode (via
    # the SSH tunnel) is not CORS-blocked. tauri.localhost = packaged WebView2;
    # localhost:7454 = desktop panel served by the wallet bridge.
    Write-Host "  ALLOWED_ORIGINS=https://${TlsDomain},http://tauri.localhost,https://tauri.localhost,http://localhost:7454"
    Write-Host "  SESSION_DB_URL=sqlite:///opt/xfchess/data/sessions.db?mode=rwc"
    Write-Host "  VAULT_DB_URL=sqlite:///opt/xfchess/data/vault.db?mode=rwc"
}

# Defensive: strip a UTF-8 BOM from .env if present. systemd's EnvironmentFile
# silently ignores the first line when it starts with a BOM (this took the backend down).
Run-Remote "sed -i '1s/^\xEF\xBB\xBF//' /opt/xfchess/.env 2>/dev/null || true"

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
    # `nginx -t` alone only checks syntax — it never applies the newly uploaded
    # config, so the ACME HTTP-01 challenge was hitting whatever config was
    # already loaded in memory (stale routing, wrong headers). Reload first so
    # the live server actually serves the /.well-known/acme-challenge/ webroot
    # this new config defines. This will 502/whatever on HTTPS momentarily since
    # no cert exists yet for this domain — fine, certbot only needs port 80.
    Run-Remote @"
nginx -t && systemctl reload nginx
test -f /etc/letsencrypt/live/${Domain}/fullchain.pem || \
  certbot certonly --webroot -w /var/www/certbot -d ${Domain} --non-interactive --agree-tos -m admin@${Domain} --quiet
"@
    Run-Remote "nginx -t && systemctl reload nginx && systemctl restart nginx"
    # Auto-renew hook
    # Single-quoted PS string ('' = literal '); \" becomes a literal " on the server
    # (PS5.1 native-arg passing eats bare embedded quotes)
    Run-Remote '(crontab -l 2>/dev/null | grep -v certbot; echo ''0 2 * * 1 certbot renew --quiet --post-hook \"systemctl reload nginx\"'') | crontab -'
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
# -SkipCertificateCheck only exists on PS6+; on Windows PowerShell 5.1 (this
# machine) it's a binding error and every check below "fails". Splat it on PS6+,
# and trust-all at the ServicePoint level on 5.1 (self-signed cert deploys).
$skipCert = @{}
if ($PSVersionTable.PSVersion.Major -ge 6) {
    $skipCert = @{ SkipCertificateCheck = $true }
} else {
    [System.Net.ServicePointManager]::SecurityProtocol = [System.Net.SecurityProtocolType]::Tls12
    [System.Net.ServicePointManager]::ServerCertificateValidationCallback = { $true }
}
$result = Invoke-RestMethod -Uri "${proto}://${TlsDomain}/api/user/status/11111111111111111111111111111111" @skipCert -ErrorAction SilentlyContinue
if ($result) {
    Write-Host "Backend responding: $($result | ConvertTo-Json -Compress)" -ForegroundColor Green
} else {
    Write-Host "Backend check failed — check logs: ssh ${DEST} journalctl -u xfchess-backend -n 50" -ForegroundColor Red
}
# /health now returns JSON {status, version, git_sha, timestamp}
$health = Invoke-RestMethod -Uri "${proto}://${TlsDomain}/health" @skipCert -ErrorAction SilentlyContinue
if ($health.status -eq "ok") {
    Write-Host "Health OK — running git_sha $($health.git_sha), version $($health.version)." -ForegroundColor Green
} else {
    Write-Host "Health endpoint failed — check: ssh ${DEST} journalctl -u xfchess-backend -n 50" -ForegroundColor Red
}
$ready = try { Invoke-WebRequest -Uri "${proto}://${TlsDomain}/readyz" -UseBasicParsing @skipCert -ErrorAction Stop; "200" } catch { $_.Exception.Response.StatusCode.value__ }
Write-Host "Readiness (/readyz): $ready (200 = DB reachable)" -ForegroundColor DarkGray

Write-Host "`n=== Deploy complete ===" -ForegroundColor Green
Write-Host "Frontend: https://${TlsDomain}" -ForegroundColor Cyan
Write-Host "API:      https://${TlsDomain}/api/user/status/<wallet>" -ForegroundColor Cyan
Write-Host "Logs:     ssh ${DEST} journalctl -u xfchess-backend -f" -ForegroundColor Cyan
Write-Host ""
Write-Host "Hetzner snapshot: Hetzner Cloud Console -> Servers -> ${Server} -> Snapshots -> Create snapshot" -ForegroundColor DarkGray
Write-Host "                  (Schedule weekly via Hetzner API: POST /servers/{id}/actions/create_image)" -ForegroundColor DarkGray
Write-Host "B2 backup:        Run 'rclone config' on the server once to configure b2xfchess remote." -ForegroundColor DarkGray
Write-Host "Secrets rotation: See deploy/SECRETS_ROTATION.md" -ForegroundColor DarkGray
Write-Host "DR note:          Keep an offline copy of /opt/xfchess/.env (password manager) —" -ForegroundColor DarkGray
Write-Host "                  vault.db backups are unreadable without IDENTITY_ENCRYPTION_KEY/IDENTITY_SALT." -ForegroundColor DarkGray
