# Upload XFChess Frontend to Hetzner Server
# Run with: .\upload-frontend.ps1

$ErrorActionPreference = "Stop"

$ServerIP = "178.104.55.19"
$LocalPath = "C:\Users\isich\XFChess\web-solana\dist\*"
$RemotePath = "/opt/xfchess/frontend/"

Write-Host "=== XFChess Frontend Upload ===" -ForegroundColor Green
Write-Host "Server: root@$ServerIP"
Write-Host "Source: $LocalPath"
Write-Host "Target: $RemotePath"
Write-Host ""

# Check if dist folder exists
if (-not (Test-Path "C:\Users\isich\XFChess\web-solana\dist\index.html")) {
    Write-Error "ERROR: dist folder not found or missing index.html!`nPlease build the frontend first: npm run build"
    exit 1
}

# Check if scp is available
$scp = Get-Command scp -ErrorAction SilentlyContinue
if (-not $scp) {
    Write-Error "ERROR: scp not found!`nPlease install OpenSSH client or Git for Windows."
    exit 1
}

Write-Host "Uploading frontend files..." -ForegroundColor Yellow

try {
    scp -r $LocalPath "root@${ServerIP}:${RemotePath}"
    
    if ($LASTEXITCODE -eq 0) {
        Write-Host ""
        Write-Host "=== Upload Complete! ===" -ForegroundColor Green
        Write-Host ""
        Write-Host "Next steps:" -ForegroundColor Cyan
        Write-Host "1. SSH to server: ssh root@$ServerIP"
        Write-Host "2. Run deployment: bash /root/deploy-to-hetzner.sh"
        Write-Host "3. Or if already set up: systemctl start xfchess-backend"
        Write-Host ""
        Write-Host "Visit: http://$ServerIP" -ForegroundColor Green
    } else {
        throw "SCP command failed"
    }
} catch {
    Write-Host ""
    Write-Error "Upload failed!`nMake sure you can SSH to the server first:`n  ssh root@$ServerIP"
    exit 1
}
