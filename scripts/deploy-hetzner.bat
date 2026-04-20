@echo off
setlocal

REM Hetzner VPS config - UPDATE THESE BEFORE RUNNING
set HETZNER_IP=your.hetzner.ip.address
set HETZNER_USER=root
set SSH_KEY=C:\Users\%USERNAME%\.ssh\id_rsa

REM Build backend for Linux
echo.
echo  Building backend for Linux target...
cd backend
cargo build --release --target x86_64-unknown-linux-gnu
cd ..

REM Create deployment package
echo.
echo  Creating deployment package...
set DEPLOY_DIR=deploy_temp
if exist %DEPLOY_DIR% rmdir /s /q %DEPLOY_DIR%
mkdir %DEPLOY_DIR%
mkdir %DEPLOY_DIR%\backend
mkdir %DEPLOY_DIR%\backend\keys

REM Copy binary
copy backend\target\x86_64-unknown-linux-gnu\release\backend %DEPLOY_DIR%\backend\

REM Copy .env template (user will need to fill in real values)
copy backend\.env %DEPLOY_DIR%\backend\.env

REM Copy systemd service file
if exist backend\backend.service (
    copy backend\backend.service %DEPLOY_DIR%\backend\
)

REM Upload to Hetzner
echo.
echo  Uploading to Hetzner...
scp -i %SSH_KEY% -r %DEPLOY_DIR%\* %HETZNER_USER%@%HETZNER_IP%:/tmp/xfchess_deploy/

REM Install and restart
echo.
echo  Installing on Hetzner...
plink -i %SSH_KEY% %HETZNER_USER%@%HETZNER_IP% bash -s << 'EOF'
    # Stop existing service
    systemctl stop xfchess-backend 2>/dev/null || true

    # Move to installation directory
    mkdir -p /opt/xfchess/backend
    cp -r /tmp/xfchess_deploy/backend/* /opt/xfchess/backend/
    chmod +x /opt/xfchess/backend/backend

    # Install systemd service if not exists
    if [ ! -f /etc/systemd/system/xfchess-backend.service ]; then
        if [ -f /opt/xfchess/backend/backend.service ]; then
            cp /opt/xfchess/backend/backend.service /etc/systemd/system/
            systemctl daemon-reload
        else
            echo "No backend.service found, skipping systemd setup"
        fi
    fi

    # Start service if systemd file exists
    if [ -f /etc/systemd/system/xfchess-backend.service ]; then
        systemctl start xfchess-backend
        systemctl enable xfchess-backend
        systemctl status xfchess-backend
    else
        echo "Starting backend manually..."
        cd /opt/xfchess/backend
        nohup ./backend > backend.log 2>&1 &
    fi
EOF

REM Cleanup
echo.
echo  Cleaning up...
rmdir /s /q %DEPLOY_DIR%
plink -i %SSH_KEY% %HETZNER_USER%@%HETZNER_IP% rm -rf /tmp/xfchess_deploy

echo.
echo  ============================================================
echo  Deployment complete!
echo  Backend should be running on http://%HETZNER_IP%:8090
echo  Check logs: ssh -i %SSH_KEY% %HETZNER_USER%@%HETZNER_IP% journalctl -u xfchess-backend -f
echo  ============================================================
echo.

endlocal
pause
