@echo off
setlocal

echo.
echo  ============================================================
echo   XFChess  ^|  Upload Frontend to Hetzner
echo  ============================================================
echo.

REM Hetzner VPS Configuration
set HETZNER_IP=178.104.55.19
set HETZNER_USER=root

cd /d "%~dp0.."
set ROOT=%cd%

REM Check if dist folder exists
if not exist "%ROOT%\web-solana\dist\" (
    echo  Building web-solana first...
    pushd "%ROOT%\web-solana"
    call npm run build
    if %ERRORLEVEL% neq 0 (
        echo  ERROR: Frontend build failed!
        pause
        exit /b 1
    )
    popd
)

echo  [1/3] Checking SSH connectivity...
ssh -o ConnectTimeout=5 -q %HETZNER_USER%@%HETZNER_IP% exit 2>nul
if %ERRORLEVEL% neq 0 (
    echo  ERROR: Cannot connect to Hetzner server!
    echo  Make sure you have SSH access: ssh %HETZNER_USER%@%HETZNER_IP%
    pause
    exit /b 1
)

echo  [2/3] Creating frontend directory...
ssh %HETZNER_USER%@%HETZNER_IP% "mkdir -p /opt/xfchess/frontend"

echo  [3/3] Uploading frontend files...
scp -r "%ROOT%\web-solana\dist\*" %HETZNER_USER%@%HETZNER_IP%:/opt/xfchess/frontend/
if %ERRORLEVEL% neq 0 (
    echo  ERROR: Upload failed!
    pause
    exit /b 1
)

echo.
echo  ============================================================
echo   Frontend Upload Complete!
echo  ============================================================
echo.
echo  Files uploaded to: /opt/xfchess/frontend/
echo  Access via:        http://%HETZNER_IP%/
echo.

pause
endlocal
