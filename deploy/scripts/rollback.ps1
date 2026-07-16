# XFChess rollback script
# Restores the previous backend binary (and optionally the databases).
# Usage: .\deploy\rollback.ps1 -Server 178.104.55.19 -User root [-RestoreDb]

param(
    [string]$Server    = "178.104.55.19",
    [string]$User      = "root",
    [switch]$RestoreDb = $false   # pass -RestoreDb to also roll back the databases
)

$DEST = "${User}@${Server}"

Write-Host "`n=== XFChess Rollback ===" -ForegroundColor Red

# ── Binary rollback ───────────────────────────────────────────────────────────
$check = & ssh $DEST "test -f /opt/xfchess/signing-server-http.prev && echo yes || echo no"
if ($check -ne "yes") {
    Write-Host "No binary backup found. Cannot roll back — this may be the first deploy." -ForegroundColor Red
    exit 1
}

Write-Host "Stopping service..." -ForegroundColor Yellow
& ssh $DEST "systemctl stop xfchess-backend"

Write-Host "Restoring previous binary..." -ForegroundColor Yellow
& ssh $DEST "cp /opt/xfchess/signing-server-http.prev /opt/xfchess/signing-server-http && chmod +x /opt/xfchess/signing-server-http"
if ($LASTEXITCODE -ne 0) { Write-Host "Binary restore failed." -ForegroundColor Red; exit 1 }

# ── Database rollback (opt-in with -RestoreDb) ────────────────────────────────
if ($RestoreDb) {
    Write-Host "`nRestoring databases from latest snapshot..." -ForegroundColor Yellow

    # Find latest backup for each db
    $latestSessions = (& ssh $DEST "ls -t /opt/xfchess/backups/sessions-*.db 2>/dev/null | head -1").Trim()
    $latestVault    = (& ssh $DEST "ls -t /opt/xfchess/backups/vault-*.db    2>/dev/null | head -1").Trim()

    if ($latestSessions) {
        Write-Host "  Restoring sessions: $latestSessions" -ForegroundColor DarkYellow
        & ssh $DEST "cp $latestSessions /opt/xfchess/data/sessions.db"
    } else {
        Write-Host "  No sessions backup found — skipping." -ForegroundColor DarkGray
    }

    if ($latestVault) {
        Write-Host "  Restoring vault:    $latestVault" -ForegroundColor DarkYellow
        & ssh $DEST "cp $latestVault /opt/xfchess/data/vault.db"
    } else {
        Write-Host "  No vault backup found — skipping." -ForegroundColor DarkGray
    }

    Write-Host "  WARNING: Any user registrations/KYC since that snapshot are lost." -ForegroundColor Red
} else {
    Write-Host "`nDB not touched. Pass -RestoreDb to also restore databases." -ForegroundColor DarkGray
}

# ── Restart and verify ────────────────────────────────────────────────────────
Write-Host "`nRestarting service..." -ForegroundColor Yellow
& ssh $DEST "systemctl start xfchess-backend"
if ($LASTEXITCODE -ne 0) { Write-Host "Service restart failed — check server." -ForegroundColor Red; exit 1 }

Start-Sleep -Seconds 3
$result = Invoke-RestMethod -Uri "http://${Server}/api/user/status/11111111111111111111111111111111" -ErrorAction SilentlyContinue
if ($result) {
    Write-Host "Rollback successful — backend responding." -ForegroundColor Green
} else {
    Write-Host "Rollback done but API not responding — check: ssh ${DEST} journalctl -u xfchess-backend -n 50" -ForegroundColor Yellow
}
$healthResult = Invoke-RestMethod -Uri "http://${Server}/health" -ErrorAction SilentlyContinue
if ($healthResult -eq "OK") {
    Write-Host "Health endpoint check passed after rollback." -ForegroundColor Green
} else {
    Write-Host "Health endpoint check failed after rollback — check: ssh ${DEST} journalctl -u xfchess-backend -n 50" -ForegroundColor Yellow
}
