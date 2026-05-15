@echo off
echo  Starting XFChess Tournament Admin Panel (Optimized)...

echo Current directory: %CD%
echo Script location: %~dp0
echo Target directory: %~dp0..\tauri\tournament-admin

pushd "%~dp0..\tauri\tournament-admin"

echo Changed to directory: %CD%
echo Looking for package.json in: %CD%

if exist package.json (
    echo  package.json found
) else (
    echo  package.json NOT found
    dir
)

echo  Installing dependencies if needed...
call npm install --silent

echo  Clearing ports 7454 and 8090...
for /f "tokens=5" %%a in ('netstat -aon ^| findstr :7454 ^| findstr LISTENING') do taskkill /f /pid %%a 2>nul
for /f "tokens=5" %%a in ('netstat -aon ^| findstr :8090 ^| findstr LISTENING') do taskkill /f /pid %%a 2>nul

echo  Starting Backend Engine in new window...
start "XFChess Backend" cmd /k "cd /d %~dp0..\backend && set SIGNING_PORT=8090 && set RUST_LOG=info && set ALLOWED_ORIGINS=http://localhost:7454 && cargo run --bin signing-server-http"

echo  Starting development server with optimizations...
set NODE_ENV=development
set VITE_CJS_IGNORE_WARNING=true
start /B cmd /c "npm run dev -- --host --port 7454"

echo ⏳ Waiting for systems to initialize...
ping 127.0.0.1 -n 6 > nul

echo  Launching Tauri desktop app...
cd /d "%~dp0..\tauri"
cargo run --features tournament-admin -- --window tournament-admin

popd
pause

