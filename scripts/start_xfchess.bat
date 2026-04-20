@echo off
setlocal

echo.
echo  ============================================================
echo   XFChess  ^|  PRODUCTION Launcher
echo  ============================================================
echo.

cd /d "%~dp0.."
set ROOT=%cd%

REM --- Step 1: Set Hetzner Backend URL ---
echo [1/8] Setting backend URLs...
set HETZNER_IP=178.104.55.19
echo  Game Backend:       http://%HETZNER_IP%/ (Hetzner)
echo  Tournament Backend: http://%HETZNER_IP%:8090 (Hetzner)

REM --- Step 2: Kill any running instances ---
echo.
echo [2/8] Cleaning up old instances...
taskkill /IM xfchess.exe             /F >nul 2>&1
taskkill /IM xfchess-tauri.exe       /F >nul 2>&1
taskkill /IM signing-server-http.exe /F >nul 2>&1
taskkill /IM vps_admin.exe           /F >nul 2>&1
timeout /t 1 /nobreak >nul

REM --- Step 3: Ensure Stockfish is available ---
echo.
echo [3/8] Ensuring Stockfish is available...
if not exist "%ROOT%\stockfish.exe" (
    echo  WARNING: stockfish.exe not found! AI will not work.
) else (
    if not exist "%ROOT%\target\release" mkdir "%ROOT%\target\release"
    copy /Y "%ROOT%\stockfish.exe" "%ROOT%\target\release\stockfish.exe" >nul
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

REM --- Step 7: Build xfchess-tauri ---
echo.
echo [7/8] Building xfchess-tauri in RELEASE mode...
cargo build -p xfchess-tauri --release --quiet
if %ERRORLEVEL% neq 0 (
    echo  ERROR: Tauri build failed.
    pause
    exit /b 1
)

REM --- Step 8: Backend Configuration ---
echo.
echo  [8/8] Using Hetzner backend exclusively (no local servers)
echo  All auth, tournament, and game services will connect to Hetzner.
echo.
echo  Hetzner Services:
echo   - Game Backend:    http://%HETZNER_IP%:80/
echo   - Auth/Tournament: http://%HETZNER_IP%:8090/

REM --- Step 10: Generate API Key if not set ---
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

REM --- Step 11: Start tournament services ---
echo.
echo  ============================================================
echo   Build complete! Starting XFChess (Hetzner Mode)...
echo  ============================================================
echo.
echo  Configuration: ALL services on Hetzner (no local servers)
echo.
echo  ============================================================
echo   Hetzner Endpoints:
echo.
echo   Game Backend:       http://%HETZNER_IP%/ (port 80)
echo   Auth API:           http://%HETZNER_IP%:8090/api/auth/*
echo   Tournament API:     http://%HETZNER_IP%:8090/api/tournament/*
echo   API Key:            %ADMIN_API_KEY%
echo  ============================================================
echo.

REM --- Launch Tauri ---
echo.
echo  Launching XFChess...
set SIGNING_SERVICE_URL=http://178.104.55.19:8090
set BACKEND_URL=http://178.104.55.19
"%ROOT%\target\release\xfchess-tauri.exe"
if %ERRORLEVEL% neq 0 (
    echo.
    echo  ERROR: XFChess exited with code %ERRORLEVEL%
    echo  Check the output above for error details.
    pause
    exit /b %ERRORLEVEL%
)

echo.
echo  XFChess process finished.
echo.
pause
endlocal
