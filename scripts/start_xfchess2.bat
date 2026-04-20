@echo off
setlocal

echo.
echo  ============================================================
echo   XFChess  ^|  Dual Instance Launcher
echo  ============================================================
echo.

cd /d "%~dp0.."
set ROOT=%cd%

REM --- Step 1: Set Hetzner Backend URL ---
echo [1/8] Setting backend URLs...
set HETZNER_IP=178.104.55.19
echo  Game Backend:       http://%HETZNER_IP%/ (Hetzner)
echo  Tournament Backend: http://%HETZNER_IP%:8090 (Hetzner)

REM --- Step 2: Kill running instances ---
echo.
echo [2/8] Cleaning up old instances...
taskkill /IM xfchess.exe             /F >nul 2>&1
taskkill /IM xfchess-tauri.exe       /F >nul 2>&1
taskkill /IM signing-server-http.exe /F >nul 2>&1
taskkill /IM vps_admin.exe           /F >nul 2>&1
timeout /t 1 /nobreak >nul

REM --- Step 3: Copy Stockfish to release dir if needed ---
echo.
echo [3/8] Ensuring Stockfish is available...
if exist "%ROOT%\stockfish.exe" (
    if not exist "%ROOT%\target\release" mkdir "%ROOT%\target\release"
    copy /Y "%ROOT%\stockfish.exe" "%ROOT%\target\release\stockfish.exe" >nul 2>&1
) else (
    echo  WARNING: stockfish.exe not found! AI will not work.
)

REM --- Check Prerequisites ---
echo.
echo [4/8] Checking prerequisites...
where npm >nul 2>&1
if %ERRORLEVEL% neq 0 (
    echo  ERROR: npm not found. Please install Node.js.
    pause
    exit /b 1
)
where cargo >nul 2>&1
if %ERRORLEVEL% neq 0 (
    echo  ERROR: cargo not found. Please install Rust.
    pause
    exit /b 1
)
echo  ✓ npm and cargo found

REM --- Step 5: Build web-solana site ---
echo.
echo [5/8] Building web-solana site...
pushd web-solana
call npm run build >nul 2>&1
if %ERRORLEVEL% neq 0 (
    echo  ERROR: web-solana build failed.
    pause
    exit /b 1
)
popd

REM --- Step 6: Build wallet onboarding UI ---
echo.
echo [6/8] Building wallet onboarding UI...
pushd tauri\wallet-ui
call npm run build >nul 2>&1
if %ERRORLEVEL% neq 0 (
    echo  ERROR: Wallet UI build failed.
    pause
    exit /b 1
)
popd

REM --- Step 7: Build xfchess binary ---
echo.
echo [7/8] Building xfchess in RELEASE mode...
cargo build --bin xfchess --features solana --release --quiet
if %ERRORLEVEL% neq 0 (
    echo  ERROR: Game build failed.
    pause
    exit /b 1
)

REM --- Step 8: Build xfchess-tauri ---
echo.
echo [8/8] Building xfchess-tauri in RELEASE mode...
cargo build -p xfchess-tauri --release --quiet
if %ERRORLEVEL% neq 0 (
    echo  ERROR: Tauri build failed.
    pause
    exit /b 1
)

REM --- Skipped: Backend HTTP server (using Hetzner) ---
echo.
echo  Skipping local backend build (using Hetzner at %HETZNER_IP%)

REM --- Skipped: VPS Admin (using Hetzner) ---
echo.
echo  Skipping local VPS Admin build (using Hetzner)

REM --- Step 11: Generate API Key if not set ---
echo.
echo  Checking for ADMIN_API_KEY...
if not defined ADMIN_API_KEY (
    echo  Generating new API key...
    for /f "tokens=*" %%i in ('powershell -Command "[guid]::NewGuid().ToString()"') do set ADMIN_API_KEY=%%i
    echo  API Key generated: %ADMIN_API_KEY%
    echo  Saving to backend\.env...
    if not exist "%ROOT%\backend\.env" (
        echo ADMIN_API_KEY=%ADMIN_API_KEY% > "%ROOT%\backend\.env"
    ) else (
        echo ADMIN_API_KEY=%ADMIN_API_KEY% >> "%ROOT%\backend\.env"
    )
) else (
    echo  Using existing API key.
)

REM --- Step 12: Start tournament services ---
echo.
echo  ============================================================
echo   Build complete! Starting XFChess with Wallet Signing...
echo  ============================================================
echo.
echo  Connecting to Hetzner backend (no local servers)...

REM --- Launch two Tauri instances with different wallet ports ---
echo.
echo  Starting Player 1 (Wallet Port 7454)...
set XFCHESS_WALLET_PORT=7454
start "XFChess Player 1" /D "%ROOT%" cmd /k "target\release\xfchess-tauri.exe || (echo [ERROR] Player 1 crashed & pause)"
timeout /t 2 /nobreak >nul

echo  Starting Player 2 (Wallet Port 7455)...
set XFCHESS_WALLET_PORT=7455
start "XFChess Player 2" /D "%ROOT%" cmd /k "target\release\xfchess-tauri.exe || (echo [ERROR] Player 2 crashed & pause)"

echo.
echo  ============================================================
echo   Two Tauri Instances Running with Wallet Signing!
echo.
echo   Player 1:        Wallet Port 7454
echo   Player 2:        Wallet Port 7455
echo   Game Backend:    http://%HETZNER_IP%/ (Hetzner)
echo   Tournament API:  http://%HETZNER_IP%:8090 (Hetzner)
echo   API Key:         %ADMIN_API_KEY%
echo  ============================================================
echo.
echo  Both instances launched. Each will open the production site.
echo  Connect wallets and click 'Launch Game' to start P2P games.
echo  Press any key to close this launcher window...
echo  (Tauri windows will stay open)
pause >nul

endlocal
