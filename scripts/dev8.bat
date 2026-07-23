@echo off
setlocal EnableDelayedExpansion

echo XFChess Dev8 - 8-Player Tournament Test Launcher
echo --------------------------------------------------
echo Launches 8 isolated game instances for tournament testing.
echo Each instance has its own P2P identity and port.
echo Start run_offline.bat first to have the backend running.
echo.

set SCRIPT_DIR=%~dp0
for %%i in ("%SCRIPT_DIR%..") do set "ROOT=%%~fi"
set "RELEASE_DIR=%ROOT%\target\debug"
set "DATA_ROOT=%ROOT%\dev8_data"

:: --- Shared environment (mirrors run_offline.bat) ---
set BACKEND_URL=http://127.0.0.1:8090
set SIGNING_SERVICE_URL=http://127.0.0.1:8090
:: No hardcoded API keys — falls back to the free public devnet RPC (same
:: fallback as the justfile). Export HELIUS_API_KEY/XFCHESS_RPC_URL yourself
:: first if you need Helius throughput locally.
if not defined XFCHESS_RPC_URL set XFCHESS_RPC_URL=https://api.devnet.solana.com
if not defined SOLANA_RPC_URL set SOLANA_RPC_URL=https://api.devnet.solana.com
if not defined HELIUS_API_KEY set HELIUS_API_KEY=
set MAGIC_BLOCK_RPC_URL=https://devnet.magicblock.app
set ER_RPC_URL=https://devnet.magicblock.app
set PROGRAM_ID=8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU
:: Non-production dummy values (same convention as the justfile) — never
:: real secrets. Export these yourself first if you need real auth locally.
if not defined JWT_SECRET set JWT_SECRET=0000000000000000000000000000000000000000000000000000000000000000
if not defined IDENTITY_ENCRYPTION_KEY set IDENTITY_ENCRYPTION_KEY=0000000000000000000000000000000000000000000000000000000000000000
if not defined IDENTITY_SALT set IDENTITY_SALT=1111111111111111111111111111111111111111111111111111111111111111

:: --- Check backend is reachable ---
curl -s --max-time 2 http://127.0.0.1:8090/health >nul 2>&1
if !errorlevel! neq 0 (
    echo [WARN] Backend not responding on :8090.
    echo        Run scripts\run_offline.bat first, then re-run dev8.bat.
    echo        Launching anyway — instances will reconnect when backend starts.
    echo.
)

:: --- Build once ---
echo [BUILD] Building xfchess with Solana features...
cd /d "%ROOT%"
cargo build --bin xfchess --features solana
if !errorlevel! neq 0 (
    echo [ERROR] Build failed.
    pause
    exit /b 1
)
echo [BUILD] Done.
echo.

:: --- Kill stale dev8 instances ---
echo [CLEANUP] Killing stale xfchess instances...
taskkill /F /IM xfchess.exe >nul 2>&1
timeout /t 1 /nobreak >nul

:: --- Create per-player data dirs ---
for /l %%i in (1,1,8) do (
    mkdir "%DATA_ROOT%\player%%i" >nul 2>&1
)

:: --- Launch 8 instances ---
echo [LAUNCH] Starting 8 instances (ports 5001-5008)...
echo          Each window = one tournament participant.
echo.

for /l %%i in (1,1,8) do (
    set /a PORT=5000+%%i
    set "PDATA=%DATA_ROOT%\player%%i"

    start "XFChess P%%i [port !PORT!]" /D "%ROOT%" cmd /k ^
        "set APPDATA=!PDATA! && ^
         set XFCHESS_P2P_PORT=!PORT! && ^
         set BACKEND_URL=%BACKEND_URL% && ^
         set SIGNING_SERVICE_URL=%SIGNING_SERVICE_URL% && ^
         set XFCHESS_RPC_URL=%XFCHESS_RPC_URL% && ^
         set SOLANA_RPC_URL=%SOLANA_RPC_URL% && ^
         set HELIUS_API_KEY=%HELIUS_API_KEY% && ^
         set MAGIC_BLOCK_RPC_URL=%MAGIC_BLOCK_RPC_URL% && ^
         set ER_RPC_URL=%ER_RPC_URL% && ^
         set PROGRAM_ID=%PROGRAM_ID% && ^
         set JWT_SECRET=%JWT_SECRET% && ^
         set IDENTITY_ENCRYPTION_KEY=%IDENTITY_ENCRYPTION_KEY% && ^
         set IDENTITY_SALT=%IDENTITY_SALT% && ^
         "%RELEASE_DIR%\xfchess.exe" --p2p-port !PORT! --log-file "%DATA_ROOT%\player%%i\game.log""

    timeout /t 1 /nobreak >nul
)

echo.
echo ========================================
echo Dev8 Environment Ready
echo ========================================
echo 8 instances launched on ports 5001-5008
echo Player data:  %DATA_ROOT%\player1..8\
echo   - node_key  (unique Iroh P2P identity per player)
echo   - game.log  (per-player log)
echo.
echo Next steps:
echo   1. Sign each instance in with a different wallet / test account
echo   2. Open Tournament Admin: Tauri tray icon -^> Tournament Admin
echo   3. Create ^& start the tournament from the admin panel
echo   4. Each instance joins via the Tournaments menu
echo ========================================
echo.
endlocal
