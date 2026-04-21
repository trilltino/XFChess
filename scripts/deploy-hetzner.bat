@echo off
setlocal

echo.
echo  ============================================================
echo   XFChess  ^|  Hetzner VPS Deploy (Windows)
echo  ============================================================
echo.

REM Hetzner VPS Configuration - UPDATE THESE IF NEEDED
set HETZNER_IP=178.104.55.19
set HETZNER_USER=root

cd /d "%~dp0.."
set ROOT=%cd%

echo  [1/5] Building backend for Linux target...
cd backend
cargo build --release --target x86_64-unknown-linux-gnu --quiet
if %ERRORLEVEL% neq 0 (
    echo  ERROR: Backend build failed!
    pause
    exit /b 1
)
cd ..

echo  [2/5] Creating deployment package...
set DEPLOY_DIR=%TEMP%\xfchess_deploy
if exist %DEPLOY_DIR% rmdir /s /q %DEPLOY_DIR%
mkdir %DEPLOY_DIR%
mkdir %DEPLOY_DIR%\backend
mkdir %DEPLOY_DIR%\keys

REM Copy binary
copy backend\target\x86_64-unknown-linux-gnu\release\backend %DEPLOY_DIR%\backend\

REM Copy .env if exists
if exist backend\.env (
    copy backend\.env %DEPLOY_DIR%\backend\
)

REM Copy keys
if exist keys\fee-payer.json copy keys\fee-payer.json %DEPLOY_DIR%\keys\
if exist keys\vps-authority.json copy keys\vps-authority.json %DEPLOY_DIR%\keys\
if exist keys\kyc-authority.json copy keys\kyc-authority.json %DEPLOY_DIR%\keys\

echo  [3/5] Uploading to Hetzner...
ssh %HETZNER_USER%@%HETZNER_IP% "mkdir -p /tmp/xfchess_deploy"
scp -r %DEPLOY_DIR%\* %HETZNER_USER%@%HETZNER_IP%:/tmp/xfchess_deploy/

echo  [4/5] Installing on server...
ssh %HETZNER_USER%@%HETZNER_IP% "bash -s" < scripts\install-on-server.sh

echo  [5/5] Cleaning up...
rmdir /s /q %DEPLOY_DIR%
ssh %HETZNER_USER%@%HETZNER_IP% "rm -rf /tmp/xfchess_deploy"

echo.
echo  ============================================================
echo   Deployment Complete!
echo  ============================================================
echo.
echo  Backend: http://%HETZNER_IP%:8090
echo.
echo  Check logs: ssh %HETZNER_USER%@%HETZNER_IP% "journalctl -u xfchess-backend -f"
echo.

pause
endlocal
