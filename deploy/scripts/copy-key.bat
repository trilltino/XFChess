@echo off
REM Copy SSH key to server
set "KEY=%USERPROFILE%\.ssh\id_xfchess.pub"
set "SERVER=root@178.104.55.19"

echo Copying public key to server...
type "%KEY%" | ssh -o StrictHostKeyChecking=accept-new %SERVER% "mkdir -p ~/.ssh && cat >> ~/.ssh/authorized_keys && chmod 600 ~/.ssh/authorized_keys"
if errorlevel 1 (
    echo Failed to copy key. Check password.
    exit /b 1
)
echo Key copied successfully.
