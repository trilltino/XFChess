@echo off
setlocal EnableDelayedExpansion

echo XFChess Local Fullstack Launcher
echo --------------------------------------------------
echo Starts the new 3D-board main menu + all backend services.
echo In-game: press K to toggle between the new menu and the classic layout.
echo.

set SCRIPT_DIR=%~dp0
for %%i in ("%SCRIPT_DIR%..") do set "ROOT=%%~fi"
set "RELEASE_DIR=%ROOT%\target\debug"

:: Check for Windows Terminal (wt.exe) to use tabs instead of separate windows
where wt >nul 2>&1
if !errorlevel! equ 0 (
    set "LAUNCH_CMD=wt -w 0 nt"
    set "DIR_FLAG=-d"
    set "TITLE_FLAG=--title"
    echo [INFO] Windows Terminal detected. Using tabs for services.
) else (
    set "LAUNCH_CMD=start"
    set "DIR_FLAG=/D"
    set "TITLE_FLAG="
    echo [INFO] Windows Terminal not found. Falling back to separate windows.
)

:: --- Build Optimizations ---
set CARGO_PROFILE_RELEASE_LTO=true
set CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1
set CARGO_PROFILE_RELEASE_OPT_LEVEL=3
set CARGO_PROFILE_RELEASE_STRIP=false
set RUSTFLAGS=-C target-cpu=native

:: --- Environment Configuration ---
set XFCHESS_FAST_LOCAL_BUILD=1
set BACKEND_URL=http://127.0.0.1:8090
set SIGNING_SERVICE_URL=http://127.0.0.1:8090

:: Secrets
set JWT_SECRET=137a895ebd9506dad79ba1f6c7d1119ad1446f7214710d93a0743f72deb5b5f3
set IDENTITY_ENCRYPTION_KEY=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef
set IDENTITY_SALT=abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789

:: Solana / Authority Keys (UPDATED to new Program ID)
set SOLANA_RPC_URL=https://beta.helius-rpc.com/?api-key=5bb5fed2-8d33-458b-b7d2-3d18fdbb3da5
set HELIUS_API_KEY=5bb5fed2-8d33-458b-b7d2-3d18fdbb3da5
set MAGIC_BLOCK_RPC_URL=https://devnet.magicblock.app
set ER_RPC_URL=https://devnet.magicblock.app
set PROGRAM_ID=8tevgspityTTG45KvvRtWV4GZ2kuGDBYWMXouFGquyDU
set FEE_PAYER_KEYS=61DHPK2JnVmdw4hLAzfjAmStMmh5S6xyw1VHNMXroAPf3CpaTuVLUKLtVoU3syinaiERTM7tHyebaUsNTXgPAgPi
set VPS_AUTHORITY_KEY=61DHPK2JnVmdw4hLAzfjAmStMmh5S6xyw1VHNMXroAPf3CpaTuVLUKLtVoU3syinaiERTM7tHyebaUsNTXgPAgPi
set KYC_AUTHORITY_KEY=61DHPK2JnVmdw4hLAzfjAmStMmh5S6xyw1VHNMXroAPf3CpaTuVLUKLtVoU3syinaiERTM7tHyebaUsNTXgPAgPi
set TOURNAMENT_FEE_RECIPIENT=uLgR6Nx4KqQobj6e2mQUPeWQpMUauDRc2oz6wZg3Y6C

:: --- Kill stale processes from previous run ---
echo [CLEANUP] Killing stale XFChess processes...
:: Kill backend by PID file first (precise), then fall back to port owner
if exist "%ROOT%\backend\.backend.pid" (
    set /p OLD_PID=<"%ROOT%\backend\.backend.pid"
    taskkill /F /PID !OLD_PID! >nul 2>&1
    del "%ROOT%\backend\.backend.pid" >nul 2>&1
)
:: Kill anything still holding port 8090
for /f "tokens=5" %%a in ('netstat -aon 2^>nul ^| findstr " :8090 "') do taskkill /F /PID %%a >nul 2>&1
:: Kill game and tauri by port 8091 (tauri IPC) and by name as last resort
taskkill /F /IM xfchess.exe >nul 2>&1
taskkill /F /IM xfchess-tauri.exe >nul 2>&1
timeout /t 1 /nobreak >nul

:: --- Build Services ---
echo [BUILD] 1/3 Building Backend Server...
cd /d "%ROOT%\backend"
cargo build --bin signing-server
if !errorlevel! neq 0 exit /b 1

echo [BUILD] 2/3 Checking UI Assets...
cd /d "%ROOT%"
if not exist "tauri\wallet-ui\dist" (
    echo [BUILD] Building Wallet UI...
    if "!LAUNCH_CMD!"=="start" (
        cd tauri\wallet-ui && npm install && npm run build && cd ..\..
    ) else (
        wt -w 0 nt --title "Build: Wallet UI" cmd /c "cd tauri\wallet-ui && npm install && npm run build"
    )
)

if not exist "tauri\tournament-admin\dist" (
    echo [BUILD] Building Admin UI...
    if "!LAUNCH_CMD!"=="start" (
        cd tauri\tournament-admin && npm install && npm run build && cd ..\..
    ) else (
        wt -w 0 nt --title "Build: Admin UI" cmd /c "cd tauri\tournament-admin && npm install && npm run build"
    )
)

echo [BUILD] 3/3 Building Game ^& Tauri Host...
cd /d "%ROOT%"
cargo build --bin xfchess --features solana
if !errorlevel! neq 0 exit /b 1

cargo build -p xfchess-tauri --features all
if !errorlevel! neq 0 (
    echo Tauri build failed.
    pause
    exit /b 1
)

:: --- Launch Services ---
echo [LAUNCH] 1/5 Monitoring Stack...
cd /d "%ROOT%\deploy\monitoring"
docker-compose -f docker-compose.local.yml ps >nul 2>&1
if !errorlevel! neq 0 (
    echo Starting local monitoring stack...
    docker-compose -f docker-compose.local.yml up -d
    timeout /t 5 /nobreak >nul
)
cd /d "%ROOT%"

echo [LAUNCH] 2/5 Signing Server...
if "!LAUNCH_CMD!"=="start" (
    start "XFChess Backend" /D "%ROOT%\backend" cmd /k "^"%RELEASE_DIR%\signing-server.exe^" 2>&1"
) else (
    wt -w 0 nt --title "XFChess Backend" -d "%ROOT%\backend" cmd /k "^"%RELEASE_DIR%\signing-server.exe^" 2>&1"
)

timeout /t 2 /nobreak >nul

echo [LAUNCH] 3/5 Tauri Host (Wallet Bridge)...
set XFCHESS_WALLET_MODE=tauri
if "!LAUNCH_CMD!"=="start" (
    start "XFChess Tauri" /D "%ROOT%" /MIN cmd /k "^"%RELEASE_DIR%\xfchess-tauri.exe^""
) else (
    wt -w 0 nt --title "XFChess Tauri" -d "%ROOT%" cmd /k "^"%RELEASE_DIR%\xfchess-tauri.exe^""
)

timeout /t 2 /nobreak >nul

echo [LAUNCH] 4/5 XFChess Game...
start "XFChess Game" /D "%ROOT%" cmd /k "^"%RELEASE_DIR%\xfchess.exe^""

echo [LAUNCH] 5/5 Web Frontend...
:: Tournament admin is desktop-only: the Tauri host serves the built dist in its
:: own window (tray icon -> Tournament Admin).
if "!LAUNCH_CMD!"=="start" (
    start "XFChess Web" /D "%ROOT%\web-solana" cmd /k "npm run dev"
) else (
    wt -w 0 nt --title "XFChess Web" -d "%ROOT%\web-solana" cmd /k "npm run dev"
)

echo.
echo ========================================
echo XFChess Local Environment Ready
echo ========================================
echo Backend:        http://127.0.0.1:8090
echo Web Frontend:   http://localhost:5173
echo Tournament Admin: desktop window (Tauri tray icon -^> Tournament Admin)
echo Program ID:     %PROGRAM_ID%
echo ========================================
echo.
endlocal
