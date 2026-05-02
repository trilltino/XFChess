# Frontend-only deploy to Hetzner
# Usage: .\deploy\deploy-frontend.ps1
# Builds web-solana and uploads dist/ to /opt/xfchess/web on the server.

param(
    [string]$Server = "178.104.55.19",
    [string]$User   = "root"
)

$SSH_KEY  = "$env:USERPROFILE\.ssh\id_xfchess"
$SSH_ARGS = @('-i', $SSH_KEY, '-o', 'StrictHostKeyChecking=accept-new')
$DEST     = "${User}@${Server}"
$ROOT     = Split-Path $PSScriptRoot -Parent

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

# ── Build ──────────────────────────────────────────────────────────────────────
Write-Host "`n=== Building frontend ===" -ForegroundColor Green
Push-Location "$ROOT\web-solana"
"VITE_BACKEND_URL=http://${Server}" | Out-File -Encoding utf8 ".env.production"
npm run build
if ($LASTEXITCODE -ne 0) { Pop-Location; throw "Frontend build failed" }
Pop-Location

# ── Upload ─────────────────────────────────────────────────────────────────────
Write-Host "`n=== Uploading to server ===" -ForegroundColor Green
Run-Remote "chmod o+x /opt/xfchess && mkdir -p /opt/xfchess/web && rm -rf /opt/xfchess/web/*"
Upload "$ROOT\web-solana\dist\*" "/opt/xfchess/web/"

# ── Fix permissions ────────────────────────────────────────────────────────────
Write-Host "`n=== Fixing permissions ===" -ForegroundColor Green
Run-Remote "chmod -R 755 /opt/xfchess/web && chown -R www-data:www-data /opt/xfchess/web"

# ── Reload nginx ───────────────────────────────────────────────────────────────
Write-Host "`n=== Reloading nginx ===" -ForegroundColor Green
Run-Remote "nginx -t && systemctl reload nginx"

Write-Host "`n=== Done ===" -ForegroundColor Green
Write-Host "Frontend live at: http://${Server}" -ForegroundColor Cyan
