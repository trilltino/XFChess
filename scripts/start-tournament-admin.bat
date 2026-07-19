@echo off
echo Starting XFChess Tournament Admin (desktop app)...

pushd "%~dp0.."

if not exist tauri\tournament-admin\dist (
    echo [BUILD] Admin UI dist missing - building...
    pushd tauri\tournament-admin
    call npm install
    call npm run build
    popd
)

echo [CLEANUP] Clearing ports 7454 and 8090...
for /f "tokens=5" %%a in ('netstat -aon ^| findstr :7454 ^| findstr LISTENING') do taskkill /f /pid %%a 2>nul
for /f "tokens=5" %%a in ('netstat -aon ^| findstr :8090 ^| findstr LISTENING') do taskkill /f /pid %%a 2>nul

echo [LAUNCH] Backend in new window...
start "XFChess Backend" cmd /k "cd /d %~dp0..\backend && set SIGNING_PORT=8090 && set RUST_LOG=info && set ALLOWED_ORIGINS=http://localhost:7454 && cargo run --bin signing-server"

echo [LAUNCH] Tournament Admin desktop window...
set XFCHESS_OPEN_ADMIN=1
cargo run -p xfchess-tauri

popd
pause
