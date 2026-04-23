@echo off
REM XFChess full deploy script
REM Usage: deploy\deploy.bat [Server] [User]
REM Prerequisites: ssh + scp in PATH (Windows OpenSSH or Git Bash), git in PATH

setlocal enabledelayedexpansion

set "SERVER=178.104.55.19"
if not "%1"=="" set "SERVER=%1"
set "USER=root"
if not "%2"=="" set "USER=%2"

set "SSH=ssh"
set "SCP=scp"
set "DEST=!USER!@!SERVER!"
set "ROOT=%~dp0.."

echo.
echo === Git preflight checks ===

pushd "!ROOT!"

REM 1. Repo identity — must be the XFChess repo
set "remoteUrl="
for /f "tokens=*" %%i in ('git remote get-url origin 2^>^&1') do set "remoteUrl=%%i"
if not defined remoteUrl (
    echo ABORT: Could not get git remote URL
    exit /b 1
)
echo !remoteUrl! | findstr /C:"XFChess" >nul
if errorlevel 1 (
    echo ABORT: This does not look like the XFChess repository.
    echo        Remote URL: !remoteUrl!
    exit /b 1
)
echo Repo:   !remoteUrl!

REM 2. Show current branch
set "branch="
for /f "tokens=*" %%i in ('git rev-parse --abbrev-ref HEAD 2^>^&1') do set "branch=%%i"
echo Branch: !branch!

REM 3. Dirty working tree — HARD STOP
set "is_dirty="
for /f "delims=" %%i in ('git status --porcelain 2^>^&1') do set "is_dirty=1"
if defined is_dirty (
    echo.
    echo   ABORT: You have uncommitted changes. Commit or stash before deploying.
    git status --porcelain
    exit /b 1
)
echo Tree:   clean

REM 4. Remote sync — HARD STOP
git fetch --quiet 2>nul
set "behind=0"
for /f "tokens=*" %%i in ('git rev-list "HEAD..origin/!branch!" --count 2^>^&1') do set "behind=%%i"

REM Validate numeric behind count
set "is_numeric="
echo !behind!| findstr /r "^[0-9][0-9]*$" >nul && set "is_numeric=1"

if defined is_numeric (
    if !behind! gtr 0 (
        echo.
        echo   ABORT: Your branch is !behind! commit(s) behind origin/!branch!.
        echo   Run: git pull  — then deploy again.
        exit /b 1
    )
)
echo Sync:   up to date with origin/!branch!

REM 5. Show exactly what is going out
for /f "tokens=*" %%i in ('git rev-parse --short HEAD') do set "commitHash=%%i"
for /f "tokens=*" %%i in ('git log -1 --pretty="%%s"') do set "commitMsg=%%i"
for /f "tokens=*" %%i in ('git log -1 --pretty="%%an"') do set "commitAuthor=%%i"
for /f "tokens=*" %%i in ('git log -1 --pretty="%%cd" --date=format:"%%Y-%%m-%%d %%H:%%M"') do set "commitDate=%%i"
echo.
echo   Deploying commit: %commitHash%
echo   Message:  %commitMsg%
echo   Author:   %commitAuthor%
echo   Date:     %commitDate%
echo.

popd

REM ── Step 1: Build backend ─────────────────────────────────────────────────────
echo.
echo === Building backend ===
pushd %ROOT%\backend
cargo build --release --bin signing-server-http
if errorlevel 1 (
    echo Backend build failed
    exit /b 1
)
popd

REM ── Step 2: Build frontend ────────────────────────────────────────────────────
echo.
echo === Building frontend ===
pushd %ROOT%\web-solana
if not exist ".env.production" (
    echo VITE_BACKEND_URL=http://%SERVER%:8090 > .env.production
    echo Created .env.production with VITE_BACKEND_URL=http://%SERVER%:8090
)
call npm run build
if errorlevel 1 (
    echo Frontend build failed
    exit /b 1
)
popd

REM ── Step 3: Server setup ──────────────────────────────────────────────────────
echo.
echo === Setting up server ===
%SSH% %DEST% "id xfchess 2>/dev/null || adduser xfchess --disabled-password --gecos ''"
if errorlevel 1 exit /b 1
%SSH% %DEST% "mkdir -p /opt/xfchess/data /opt/xfchess/web /opt/xfchess/backups"
if errorlevel 1 exit /b 1
%SSH% %DEST% "chown -R xfchess:xfchess /opt/xfchess"
if errorlevel 1 exit /b 1
%SSH% %DEST% "apt-get update -qq && apt-get install -y -qq nginx sqlite3"
if errorlevel 1 exit /b 1

REM Install nightly cron backup (3am UTC, keeps 14 days)
set "cronJob=0 3 * * * sqlite3 /opt/xfchess/data/sessions.db \".backup '/opt/xfchess/backups/sessions-$(date +%%Y%%m%%d).db'\" ; sqlite3 /opt/xfchess/data/vault.db \".backup '/opt/xfchess/backups/vault-$(date +%%Y%%m%%d).db'\" ; find /opt/xfchess/backups -name '*.db' -mtime +14 -delete"
%SSH% %DEST% "(crontab -l 2>/dev/null | grep -v xfchess/backups; echo '%cronJob%') | crontab -"
if errorlevel 1 exit /b 1
echo Nightly backup cron installed (3am UTC, 14-day retention)

REM ── Step 4: Backup databases + binary before touching anything ───────────────
echo.
echo === Snapshotting databases ===
for /f "tokens=*" %%i in ('%SSH% %DEST% "date +%%Y%%m%%d-%%H%%M%%S"') do set "ts=%%i"
%SSH% %DEST% "mkdir -p /opt/xfchess/backups"
if errorlevel 1 exit /b 1
%SSH% %DEST% "sqlite3 /opt/xfchess/data/sessions.db \".backup '/opt/xfchess/backups/sessions-%ts%.db'\" 2>/dev/null || cp /opt/xfchess/data/sessions.db /opt/xfchess/backups/sessions-%ts%.db 2>/dev/null || true"
%SSH% %DEST% "sqlite3 /opt/xfchess/data/vault.db \".backup '/opt/xfchess/backups/vault-%ts%.db'\" 2>/dev/null || cp /opt/xfchess/data/vault.db /opt/xfchess/backups/vault-%ts%.db 2>/dev/null || true"
%SSH% %DEST% "ls -t /opt/xfchess/backups/sessions-*.db 2>/dev/null | tail -n +8 | xargs rm -f"
%SSH% %DEST% "ls -t /opt/xfchess/backups/vault-*.db 2>/dev/null | tail -n +8 | xargs rm -f"
echo DB snapshot: sessions-%ts%.db + vault-%ts%.db (7 kept)

echo.
echo === Backing up current binary ===
%SSH% %DEST% "cp /opt/xfchess/signing-server-http /opt/xfchess/signing-server-http.prev 2>/dev/null || true"
echo Binary backup saved as signing-server-http.prev

echo.
echo === Uploading backend binary ===
%SCP% -r "%ROOT%\backend\target\release\signing-server-http" "%DEST%:/opt/xfchess/signing-server-http"
if errorlevel 1 (
    echo Upload failed: backend binary
    exit /b 1
)
%SSH% %DEST% "chmod +x /opt/xfchess/signing-server-http"
if errorlevel 1 exit /b 1

REM ── Step 5a: Upload keypair files ────────────────────────────────────────────
echo.
echo === Uploading keypair files ===
%SSH% %DEST% "mkdir -p /opt/xfchess/keys && chmod 700 /opt/xfchess/keys"
if errorlevel 1 exit /b 1

set "key1=C:\Users\isich\.config\solana\id.json"
set "dest1=/opt/xfchess/keys/id.json"
if exist "%key1%" (
    %SCP% -r "%key1%" "%DEST%:%dest1%"
    if errorlevel 1 exit /b 1
    %SSH% %DEST% "chmod 600 %dest1%"
    if errorlevel 1 exit /b 1
    for %%f in ("%key1%") do echo Uploaded %%~nxf
) else (
    echo WARNING: %key1% not found — skipping
)

set "key2=C:\Users\isich\.config\xfchess\relayer-devnet.json"
set "dest2=/opt/xfchess/keys/relayer-devnet.json"
if exist "%key2%" (
    %SCP% -r "%key2%" "%DEST%:%dest2%"
    if errorlevel 1 exit /b 1
    %SSH% %DEST% "chmod 600 %dest2%"
    if errorlevel 1 exit /b 1
    for %%f in ("%key2%") do echo Uploaded %%~nxf
) else (
    echo WARNING: %key2% not found — skipping
)

REM ── Step 5: Upload .env if it exists ─────────────────────────────────────────
echo.
echo === Checking .env ===
set "envFile=%ROOT%\deploy\.env.production"
if exist "%envFile%" (
    %SCP% -r "%envFile%" "%DEST%:/opt/xfchess/.env"
    if errorlevel 1 exit /b 1
    %SSH% %DEST% "chmod 600 /opt/xfchess/.env"
    if errorlevel 1 exit /b 1
    echo .env uploaded
) else (
    echo WARNING: %envFile% not found. Create it from deploy/.env.example before the server will start.
    echo Required minimum content:
    echo   JWT_SECRET=^<openssl rand -hex 32^>
    echo   IDENTITY_ENCRYPTION_KEY=^<openssl rand -hex 32^>
    echo   IDENTITY_SALT=^<openssl rand -hex 32^>
    echo   ALLOWED_ORIGINS=http://%SERVER%
    echo   SESSION_DB_URL=sqlite:///opt/xfchess/data/sessions.db?mode=rwc
    echo   VAULT_DB_URL=sqlite:///opt/xfchess/data/vault.db?mode=rwc
)

REM ── Step 6: Upload systemd service ───────────────────────────────────────────
echo.
echo === Installing systemd service ===
%SCP% -r "%ROOT%\deploy\xfchess-backend.service" "%DEST%:/etc/systemd/system/xfchess-backend.service"
if errorlevel 1 exit /b 1
%SSH% %DEST% "systemctl daemon-reload"
if errorlevel 1 exit /b 1
%SSH% %DEST% "systemctl enable xfchess-backend"
if errorlevel 1 exit /b 1
%SSH% %DEST% "systemctl restart xfchess-backend"
if errorlevel 1 exit /b 1

REM ── Step 7: Upload frontend ───────────────────────────────────────────────────
echo.
echo === Uploading frontend ===
%SCP% -r "%ROOT%\web-solana\dist\*" "%DEST%:/opt/xfchess/web/"
if errorlevel 1 exit /b 1

REM ── Step 8: Configure nginx ───────────────────────────────────────────────────
echo.
echo === Configuring nginx ===
%SCP% -r "%ROOT%\deploy\nginx.conf" "%DEST%:/etc/nginx/sites-available/xfchess"
if errorlevel 1 exit /b 1
%SSH% %DEST% "sed -i 's/YOUR_DOMAIN/%SERVER%/g' /etc/nginx/sites-available/xfchess"
if errorlevel 1 exit /b 1
%SSH% %DEST% "sed -i '/ssl_/d; /listen 443/d; /return 301/d' /etc/nginx/sites-available/xfchess"
if errorlevel 1 exit /b 1
%SSH% %DEST% "ln -sf /etc/nginx/sites-available/xfchess /etc/nginx/sites-enabled/xfchess"
if errorlevel 1 exit /b 1
%SSH% %DEST% "rm -f /etc/nginx/sites-enabled/default"
if errorlevel 1 exit /b 1
%SSH% %DEST% "nginx -t && systemctl reload nginx"
if errorlevel 1 exit /b 1

REM ── Step 9: Verify ────────────────────────────────────────────────────────────
echo.
echo === Verifying deployment ===
timeout /t 3 /nobreak >nul
curl -s "http://%SERVER%/api/user/status/11111111111111111111111111111111" >nul 2>&1
if errorlevel 1 (
    echo Backend check failed - check logs: ssh %DEST% journalctl -u xfchess-backend -n 50
) else (
    echo Backend responding
)

echo.
echo === Deploy complete ===
echo Frontend: http://%SERVER%
echo API:      http://%SERVER%/api/user/status/^<wallet^>
echo Logs:     ssh %DEST% journalctl -u xfchess-backend -f
