@echo off
REM ─────────────────────────────────────────────────────────────────────────────
REM run_tournament_local.bat
REM Launches signing-server + 4 XFChess (Tauri) instances for tournament testing.
REM Each instance gets its own Tauri wallet port so Phantom popups don't collide.
REM Usage:  run_tournament_local.bat [--no-build]
REM ─────────────────────────────────────────────────────────────────────────────
setlocal
cd /d "%~dp0"

set BUILD=1
if "%1"=="--no-build" set BUILD=0

if %BUILD%==1 (
    echo [BUILD] Building signing-server, xfchess, xfchess-tauri...
    cargo build -p backend --bin signing-server
    if errorlevel 1 ( echo [ERROR] signing-server build failed. & pause & exit /b 1 )
    cargo build --features solana --bin xfchess
    if errorlevel 1 ( echo [ERROR] xfchess build failed. & pause & exit /b 1 )
    cargo build -p xfchess-tauri
    if errorlevel 1 ( echo [ERROR] xfchess-tauri build failed. & pause & exit /b 1 )
    echo [BUILD] Done.
)

if not exist target\debug\xfchess-tauri.exe (
    echo [ERROR] target\debug\xfchess-tauri.exe not found. Run without --no-build.
    pause & exit /b 1
)

REM ── Kill any stale processes ──────────────────────────────────────────────────
taskkill /IM xfchess-tauri.exe /F >nul 2>&1
taskkill /IM xfchess.exe /F >nul 2>&1
taskkill /IM signing-server.exe /F >nul 2>&1

REM ── Shared environment ────────────────────────────────────────────────────────
set RUST_LOG=info,wgpu_hal=off,wgpu_core=off,wgpu=off,bevy_render=error,bevy_winit=warn,bevy_diagnostic=off
set FEE_PAYER_KEYS=keys\fee-payer.json
set SIGNING_PORT=8090
set JWT_SECRET=change-me-in-production-32-bytes!!
set SOLANA_RPC_URL=https://api.devnet.solana.com
set ER_RPC_URL=https://devnet-eu.magicblock.app/
set PROGRAM_ID=FVPp29xDtMrh3CrTJNnxDcbGRnMMKuUv2ntqkBRc1uDX
set SIGNING_SERVICE_URL=http://127.0.0.1:8090
set XFCHESS_SOLANA=1

REM ── Start signing server ──────────────────────────────────────────────────────
echo [LAUNCH] Starting signing server...
start "Signing Server" target\debug\signing-server.exe
timeout /t 3 /nobreak >nul

REM ── Launch 4 Tauri instances (unique wallet port each) ────────────────────────
echo [LAUNCH] Opening 4 player windows...

set XFCHESS_WALLET_PORT=7454
start "XFChess - Player 0 (SF1 White)" target\debug\xfchess-tauri.exe
timeout /t 3 /nobreak >nul

set XFCHESS_WALLET_PORT=7464
start "XFChess - Player 1 (SF2 White)" target\debug\xfchess-tauri.exe
timeout /t 3 /nobreak >nul

set XFCHESS_WALLET_PORT=7474
start "XFChess - Player 2 (SF1 Black)" target\debug\xfchess-tauri.exe
timeout /t 3 /nobreak >nul

set XFCHESS_WALLET_PORT=7484
start "XFChess - Player 3 (SF2 Black)" target\debug\xfchess-tauri.exe

echo.
echo [OK] Signing server + 4 instances launched.
echo.
echo In each window:
echo   1. Navigate to Tournaments ^> click "Connect Wallet"
echo   2. Approve Phantom connection in the browser tab that opens
echo   3. Click Refresh, then Join Tournament
echo   4. Approve the on-chain registration popup in Phantom
echo.
echo After all 4 players registered, start bracket via admin console:
echo   anchor test --skip-local-validator -- --grep "tournament"
echo.
endlocal
