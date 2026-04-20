@echo off
REM Upload XFChess frontend to Hetzner Server
REM Requires: scp (included with Windows 10/11 or Git for Windows)

echo === Uploading XFChess Frontend to Hetzner ===
echo.
echo Server: root@178.104.55.19
echo Target: /opt/xfchess/frontend/
echo.

REM Check if dist folder exists
if not exist "C:\Users\isich\XFChess\web-solana\dist\" (
    echo ERROR: dist folder not found!
    echo Please build the frontend first with: npm run build
    exit /b 1
)

REM Upload frontend files
echo Uploading frontend files...
scp -r C:\Users\isich\XFChess\web-solana\dist\* root@178.104.55.19:/opt/xfchess/frontend/

if %ERRORLEVEL% NEQ 0 (
    echo.
    echo ERROR: Upload failed!
    echo Make sure you can SSH to the server: ssh root@178.104.55.19
    exit /b 1
)

echo.
echo === Upload Complete ===
echo.
echo Files uploaded to /opt/xfchess/frontend/
echo.
echo Next steps:
echo 1. SSH to server: ssh root@178.104.55.19
echo 2. Run the deployment script: bash /root/deploy-to-hetzner.sh
echo 3. Or if already set up, just start the service: systemctl start xfchess-backend
echo.

pause
