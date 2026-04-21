@echo off
setlocal

echo.
echo  ============================================================
echo   XFChess  ^|  Deploy to Hetzner Server
echo  ============================================================
echo.

REM Hetzner VPS Configuration
set HETZNER_IP=178.104.55.19
set HETZNER_USER=root

cd /d "%~dp0.."
set ROOT=%cd%

REM Check for GitHub token
if not defined GITHUB_TOKEN (
    echo  ERROR: GITHUB_TOKEN environment variable not set!
    echo  Please set it before running: set GITHUB_TOKEN=your_token_here
    pause
    exit /b 1
)

echo  [1/5] Checking SSH connectivity...
ssh -o ConnectTimeout=5 -q %HETZNER_USER%@%HETZNER_IP% exit 2>nul
if %ERRORLEVEL% neq 0 (
    echo  ERROR: Cannot connect to Hetzner server!
    echo  Make sure you have SSH access: ssh %HETZNER_USER%@%HETZNER_IP%
    pause
    exit /b 1
)

echo  [2/5] Stopping existing services...
ssh %HETZNER_USER%@%HETZNER_IP% "systemctl stop xfchess-backend 2>/dev/null || true"

echo  [3/5] Building backend for Linux...
cd backend
cargo build --release --target x86_64-unknown-linux-gnu --quiet
if %ERRORLEVEL% neq 0 (
    echo  ERROR: Backend build failed!
    pause
    exit /b 1
)
cd ..

echo  [4/5] Uploading to Hetzner...
ssh %HETZNER_USER%@%HETZNER_IP% "mkdir -p /opt/xfchess/backend /opt/xfchess/keys"

scp backend\target\x86_64-unknown-linux-gnu\release\backend %HETZNER_USER%@%HETZNER_IP%:/opt/xfchess/backend/
if %ERRORLEVEL% neq 0 (
    echo  ERROR: Failed to upload backend binary!
    pause
    exit /b 1
)

REM Copy keys if they exist
if exist "keys\fee-payer.json" (
    scp keys\fee-payer.json %HETZNER_USER%@%HETZNER_IP%:/opt/xfchess/keys/
)
if exist "keys\vps-authority.json" (
    scp keys\vps-authority.json %HETZNER_USER%@%HETZNER_IP%:/opt/xfchess/keys/
)
if exist "keys\kyc-authority.json" (
    scp keys\kyc-authority.json %HETZNER_USER%@%HETZNER_IP%:/opt/xfchess/keys/
)

REM Copy .env if it exists
if exist "backend\.env" (
    scp backend\.env %HETZNER_USER%@%HETZNER_IP%:/opt/xfchess/backend/
)

echo  [5/5] Setting up service and starting...
ssh %HETZNER_USER%@%HETZNER_IP% "chmod +x /opt/xfchess/backend/backend && systemctl daemon-reload && systemctl restart xfchess-backend && systemctl enable xfchess-backend"

echo.
echo  ============================================================
echo   Deployment Complete!
echo  ============================================================
echo.
echo  Server:     http://%HETZNER_IP%/
echo  Backend:    http://%HETZNER_IP%:8090/
echo.
echo  Check logs: ssh %HETZNER_USER%@%HETZNER_IP% "journalctl -u xfchess-backend -f"
echo.

pause
endlocal
