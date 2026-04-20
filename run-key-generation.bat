@echo off
REM Upload and run key generation script on Hetzner server

echo === XFChess Key Generation on Hetzner ===
echo.

REM Upload the script
echo Uploading key generation script...
scp C:\Users\isich\XFChess\generate-keys-on-server.sh root@178.104.55.19:/root/

if %ERRORLEVEL% NEQ 0 (
    echo ERROR: Upload failed!
    echo Make sure you can SSH to the server: ssh root@178.104.55.19
    pause
    exit /b 1
)

echo.
echo Running key generation script on server...
echo.

REM SSH and run the script
ssh root@178.104.55.19 "bash /root/generate-keys-on-server.sh"

echo.
echo === Key Generation Complete ===
echo.
echo Keys have been generated and .env file updated on the server.
echo.
echo IMPORTANT: Fund the fee payer wallet with devnet SOL.
echo The script will show you the command to run.
echo.
echo Key files are backed up in ~/xfchess-keys/ on the server.
echo.
pause
