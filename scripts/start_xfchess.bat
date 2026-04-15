@echo off
setlocal

echo.
echo  ============================================================
echo   XFChess  ^|  Full Build + Launch (RELEASE MODE)
echo  ============================================================
echo.

cd /d "%~dp0.."
set ROOT=%cd%

REM --- Kill any running instances ---
echo [0/6] Killing any running instances...
taskkill /IM xfchess.exe        /F >nul 2>&1
taskkill /IM xfchess-tauri.exe  /F >nul 2>&1
taskkill /IM signing-server.exe /F >nul 2>&1
taskkill /IM signing-server-http.exe /F >nul 2>&1
taskkill /IM vps_admin.exe /F >nul 2>&1
timeout /t 1 /nobreak >nul

REM --- Ensure Stockfish is available ---
echo.
echo [1/6] Ensuring Stockfish is available...
if not exist "%ROOT%\stockfish.exe" (
    echo  WARNING: stockfish.exe not found! AI will not work.
) else (
    if not exist "%ROOT%\target\release" mkdir "%ROOT%\target\release"
    copy /Y "%ROOT%\stockfish.exe" "%ROOT%\target\release\stockfish.exe" >nul
)

REM --- Build React onboarding UI ---
echo.
echo [2/6] Building web-solana site (merged into localhost:7454)...
pushd web-solana
call npm run build >nul 2>&1
if %ERRORLEVEL% neq 0 (
    echo  ERROR: web-solana build failed.
    pause
    exit /b 1
)
popd

REM --- Build wallet onboarding UI ---
echo.
echo [3/6] Building wallet onboarding UI...
pushd tauri\wallet-ui
call npm run build >nul 2>&1
if %ERRORLEVEL% neq 0 (
    echo  ERROR: UI build failed.
    pause
    exit /b 1
)
popd

REM --- Check if target builds exist ---
echo.
echo [4/6] Checking for target builds...
if exist "%ROOT%\target\release\xfchess.exe" (
    echo  Using existing xfchess.exe from target/release
) else (
    echo  Building xfchess in RELEASE mode...
    cargo build --bin xfchess --features solana --release --quiet
    if %ERRORLEVEL% neq 0 (
        echo  ERROR: Game build failed.
        pause
        exit /b 1
    )
)

if exist "%ROOT%\target\release\xfchess-tauri.exe" (
    echo  Using existing xfchess-tauri.exe from target/release
) else (
    echo  Building xfchess-tauri in RELEASE mode...
    cargo build -p xfchess-tauri --release --quiet
    if %ERRORLEVEL% neq 0 (
        echo  ERROR: Tauri build failed.
        pause
        exit /b 1
    )
)

REM --- Build backend HTTP server ---
echo.
echo [5/7] Building backend HTTP server...
if exist "%ROOT%\backend\target\release\signing-server-http.exe" (
    echo  Using existing signing-server-http.exe from backend/target/release
) else (
    echo  Building signing-server-http in RELEASE mode...
    pushd backend
    cargo build --bin signing-server-http --release --quiet
    if %ERRORLEVEL% neq 0 (
        echo  ERROR: Backend HTTP build failed.
        pause
        exit /b 1
    )
    popd
)

REM --- Build VPS Admin ---
echo.
echo [6/7] Building VPS Admin...
if exist "%ROOT%\backend\target\release\vps_admin.exe" (
    echo  Using existing vps_admin.exe from backend/target/release
) else (
    echo  Building vps_admin in RELEASE mode...
    pushd backend
    cargo build --bin vps_admin --release --quiet
    if %ERRORLEVEL% neq 0 (
        echo  ERROR: VPS Admin build failed.
        pause
        exit /b 1
    )
    popd
)

REM --- Backend URL is now compiled into binary via build.rs ---
echo.
echo [Config] Backend URL is compiled into binary (backend_url.txt).
echo  Edit backend_url.txt and rebuild to change.
echo.

REM --- Generate API Key if not set ---
echo.
echo [API Key] Checking for ADMIN_API_KEY...
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

REM --- Start Local Backend ---
echo.
echo  ============================================================
echo   Build complete! Starting Backend Server...
echo  ============================================================
echo.
echo [6/7] Starting Backend HTTP Server (port 8090)...
start "XFChess Backend" /D "%ROOT%\backend" cmd /c "set ADMIN_API_KEY=%ADMIN_API_KEY% && set SIGNING_SERVICE_URL=http://localhost:8090 && target\release\signing-server-http.exe"
echo  Backend server starting on port 8090...
timeout /t 3 /nobreak >nul

echo.
echo  ============================================================
echo   Backend Running! Starting VPS Admin Server...
echo  ============================================================
echo.

REM --- Start VPS Admin Server ---
echo [7/7] Starting VPS Admin Server with API key...
pushd backend
start "XFChess VPS Admin" /D "%ROOT%\backend" cmd /c "set ADMIN_API_KEY=%ADMIN_API_KEY% && set SIGNING_SERVICE_URL=http://localhost:8090 && cargo run --bin vps_admin --release"
popd
echo  VPS Admin server starting...
timeout /t 2 /nobreak >nul

echo.
echo  ============================================================
echo   All Services Running! Launching XFChess...
echo   Chrome opens to localhost:7454 (site + onboarding merged).
echo   Backend: http://localhost:8090
echo   VPS Admin: Running in separate window
echo  ============================================================
echo.

REM --- Launch Tauri (serves merged site + opens Chrome to /onboard) ---
echo [8/8] Starting XFChess...
"%ROOT%\target\release\xfchess-tauri.exe"

echo.
echo  XFChess process finished.
echo.
pause
endlocal
