@echo off
setlocal EnableDelayedExpansion

echo XFChess Multi-Instance Local Launcher
echo -----------------------------------
echo This script launches TWO instances of XFChess for P2P testing.
echo Instance 1: Port 7454, User: Player1
echo Instance 2: Port 7464, User: Player2
echo.

set SCRIPT_DIR=%~dp0
for %%i in ("%SCRIPT_DIR%..") do set "ROOT=%%~fi"
set "RELEASE_DIR=%ROOT%\target\debug"

:: Check for Windows Terminal (wt.exe) to use tabs instead of separate windows
where wt >nul 2>nul
if %errorlevel% equ 0 (
    set "LAUNCH_CMD=wt -w 0 nt"
    echo [INFO] Windows Terminal detected. Using tabs for services.
) else (
    set "LAUNCH_CMD=start"
    echo [INFO] Windows Terminal not found. Falling back to separate windows.
)

:: --- Environment Configuration ---
set XFCHESS_FAST_LOCAL_BUILD=1
set BACKEND_URL=http://127.0.0.1:8090
set SIGNING_SERVICE_URL=http://127.0.0.1:8090

:: Secrets from your .env
set JWT_SECRET=137a895ebd9506dad79ba1f6c7d1119ad1446f7214710d93a0743f72deb5b5f3
set IDENTITY_ENCRYPTION_KEY=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef
set IDENTITY_SALT=abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789

:: Solana / Authority Keys
set SOLANA_RPC_URL=https://api.devnet.solana.com
set MAGIC_BLOCK_RPC_URL=https://devnet.magicblock.app
set ER_RPC_URL=https://devnet.magicblock.app
set PROGRAM_ID=8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU
set FEE_PAYER_KEYS=61DHPK2JnVmdw4hLAzfjAmStMmh5S6xyw1VHNMXroAPf3CpaTuVLUKLtVoU3syinaiERTM7tHyebaUsNTXgPAgPi
set VPS_AUTHORITY_KEY=61DHPK2JnVmdw4hLAzfjAmStMmh5S6xyw1VHNMXroAPf3CpaTuVLUKLtVoU3syinaiERTM7tHyebaUsNTXgPAgPi
set KYC_AUTHORITY_KEY=61DHPK2JnVmdw4hLAzfjAmStMmh5S6xyw1VHNMXroAPf3CpaTuVLUKLtVoU3syinaiERTM7tHyebaUsNTXgPAgPi
set HOST_TREASURY_PUBKEY=uLgR6Nx4KqQobj6e2mQUPeWQpMUauDRc2oz6wZg3Y6C

:: --- Build Phase ---
echo [BUILD] Starting parallel builds...

:: 1. UI Builds (Only if dist is missing)
cd /d "%ROOT%"
if not exist "tauri\wallet-ui\dist" (
    echo [BUILD] Building Wallet UI - First time...
    wt -w 0 nt --title "Build: Wallet UI" cmd /c "cd tauri\wallet-ui && npm install && npm run build"
)
if not exist "tauri\tournament-admin\dist" (
    echo [BUILD] Building Tournament Admin UI - First time...
    wt -w 0 nt --title "Build: Admin UI" cmd /c "cd tauri\tournament-admin && npm install && npm run build"
)

:: 2. Signing Server
echo [BUILD] Building XFChess Signing Server...
cd /d "%ROOT%\backend"
cargo build --bin signing-server
if errorlevel 1 exit /b 1

:: 3. Game
echo [BUILD] Building XFChess Game...
cd /d "%ROOT%"
cargo build --bin xfchess --features solana
if errorlevel 1 exit /b 1

echo [BUILD] Building Tauri host - debug build...
cd /d "%ROOT%"
cargo build -p xfchess-tauri --features all
if errorlevel 1 exit /b 1

:: --- Launch Phase ---
echo.
set RELEASE_DIR=%ROOT%\target\debug
if "!LAUNCH_CMD!"=="start" (
    start "XFChess Backend" /D "%ROOT%\backend" cmd /k "%RELEASE_DIR%\signing-server.exe 2>&1"
) else (
    wt -w 0 nt --title "XFChess Backend" -d "%ROOT%\backend" cmd /k "%RELEASE_DIR%\signing-server.exe 2>&1"
)

timeout /t 3 /nobreak >nul

set XFCHESS_WALLET_PORT=7454
set XFCHESS_WALLET_MODE=tauri
if "!LAUNCH_CMD!"=="start" (
    start "XFChess Tauri 1" /D "%ROOT%" /MIN cmd /k "set XFCHESS_WALLET_PORT=7454 && %RELEASE_DIR%\xfchess-tauri.exe"
) else (
    wt -w 0 nt --title "Tauri 1 (7454)" -d "%ROOT%" cmd /k "set XFCHESS_WALLET_PORT=7454 && %RELEASE_DIR%\xfchess-tauri.exe"
)
timeout /t 1 /nobreak >nul
start "XFChess Game 1" /D "%ROOT%" cmd /k "set XFCHESS_WALLET_PORT=7454 && ^"%RELEASE_DIR%\xfchess.exe^""

set XFCHESS_WALLET_PORT=7464
set XFCHESS_WALLET_MODE=tauri
if "!LAUNCH_CMD!"=="start" (
    start "XFChess Tauri 2" /D "%ROOT%" /MIN cmd /k "set XFCHESS_WALLET_PORT=7464 && %RELEASE_DIR%\xfchess-tauri.exe"
) else (
    wt -w 0 nt --title "Tauri 2 (7464)" -d "%ROOT%" cmd /k "set XFCHESS_WALLET_PORT=7464 && %RELEASE_DIR%\xfchess-tauri.exe"
)
timeout /t 1 /nobreak >nul
start "XFChess Game 2" /D "%ROOT%" cmd /k "set XFCHESS_WALLET_PORT=7464 && ^"%RELEASE_DIR%\xfchess.exe^""

if "!LAUNCH_CMD!"=="start" (
    start "XFChess Web" /D "%ROOT%\web-solana" cmd /k "npm run dev"
    start "Tournament Admin" /D "%ROOT%\tauri\tournament-admin" cmd /k "npm run dev -- --port 7454"
) else (
    wt -w 0 nt --title "XFChess Web" -d "%ROOT%\web-solana" cmd /k "npm run dev"
    wt -w 0 nt --title "Tourney Admin" -d "%ROOT%\tauri\tournament-admin" cmd /k "npm run dev -- --port 7454"
)

echo [5/5] Instances are ready.
timeout /t 4 /nobreak >nul

echo.
echo ========================================
echo Multi-Instance Ready
echo ========================================
echo Instance 1: http://localhost:7454 (User: Player1)
echo Instance 2: http://localhost:7464 (User: Player2)
echo Tournament Admin: http://localhost:7454/tournament-admin/
echo Grafana:   http://localhost:3000 (admin/admin)
echo Prometheus: http://localhost:9090
echo ========================================
echo.
endlocal
