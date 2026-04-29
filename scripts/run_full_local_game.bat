@echo off
setlocal EnableDelayedExpansion

:: Title and initial setup
echo XFChess Full Local Game Launcher
echo --------------------------------
echo This script builds and launches XFChess locally with all features.
echo It starts a local backend, launches the game, and opens the site.
echo.

set SCRIPT_DIR=%~dp0
set ROOT=%SCRIPT_DIR%..
set RELEASE_DIR=%ROOT%\release\local
set WEB_DIR=%ROOT%\web-solana

:: Set environment variables for local development
set BACKEND_URL=http://localhost:8090
set SIGNING_SERVICE_URL=http://localhost:8090
set SIGNING_PORT=8090
set SESSION_DB_URL=sqlite://sessions.db?mode=rwc
set VAULT_DB_URL=sqlite://vault.db?mode=rwc
set XFCHESS_WALLET_MODE=tauri
set XFCHESS_WALLET_PORT=7454
set XFCHESS_FAST_LOCAL_BUILD=1

:: Step 1: Build the game from source
echo Building XFChess game from source...
call "%SCRIPT_DIR%package_release_local.bat"
if %ERRORLEVEL% neq 0 (
    echo Build failed. Please check the error messages above.
    pause
    exit /b %ERRORLEVEL%
)
echo Build completed successfully.
echo.

:: Step 2: Start local backend server
echo Starting local backend server on port 8090...
if exist "%RELEASE_DIR%\backend.log" del /f /q "%RELEASE_DIR%\backend.log" >nul 2>&1
start "XFChess Signing Server" /D "%RELEASE_DIR%" cmd /c "set \"SIGNING_SERVICE_URL=http://127.0.0.1:8090\" && set \"SIGNING_PORT=8090\" && set \"SESSION_DB_URL=sqlite://sessions.db?mode=rwc\" && set \"VAULT_DB_URL=sqlite://vault.db?mode=rwc\" && signing-server-http.exe > backend.log 2>&1"
if %ERRORLEVEL% neq 0 (
    echo Failed to start backend server. Please check the error messages.
    pause
    exit /b %ERRORLEVEL%
)
echo Waiting for backend to start...
set BACKEND_READY=
for /L %%I in (1,1,20) do (
    powershell -NoProfile -Command "try { $r = Invoke-WebRequest -UseBasicParsing http://127.0.0.1:8090/api/user/status/healthcheck -TimeoutSec 2; exit 0 } catch { exit 1 }" >nul 2>&1
    if !ERRORLEVEL! EQU 0 (
        set BACKEND_READY=1
        goto :backend_ready
    )
    timeout /t 1 /nobreak >nul
)
echo Backend failed to start on http://127.0.0.1:8090
if exist "%RELEASE_DIR%\backend.log" type "%RELEASE_DIR%\backend.log"
pause
exit /b 1
:backend_ready
echo Backend is responding on port 8090.

:: Step 3: Launch the game executable
echo Launching XFChess game executable...
set STOCKFISH_PATH=%RELEASE_DIR%\stockfish.exe
start "XFChess" /D "%RELEASE_DIR%" /MAX "%RELEASE_DIR%\xfchess-tauri.exe"
if %ERRORLEVEL% neq 0 (
    echo Failed to launch game executable. Please check if it was built correctly.
    pause
    exit /b %ERRORLEVEL%
)
echo Game launched successfully.
echo.

:: Step 4: Start local web server for the site
echo Starting local web server for the site on port 5173...
start "XFChess Web" /D "%WEB_DIR%" cmd /c npm run dev
if %ERRORLEVEL% neq 0 (
    echo Failed to start web server. Please ensure npm dependencies are installed.
    pause
    exit /b %ERRORLEVEL%
)
echo Waiting for web server to start...
timeout /t 5 /nobreak >nul

:: Step 5: Open the local site in default browser
echo Opening local site in browser...
start http://localhost:5173
if %ERRORLEVEL% neq 0 (
    echo Failed to open browser. Please open http://localhost:5173 manually.
    pause
    exit /b %ERRORLEVEL%
)
echo Site opened successfully.
echo.

echo XFChess full local setup complete!
echo - Game is running and connected to local backend at http://localhost:8090

echo - Site is open at http://localhost:5173 and should authenticate with the backend

echo Press Ctrl+C to stop the backend and web server when done.
pause

:: Cleanup on exit
echo Shutting down local servers...
taskkill /IM cargo.exe /F >nul 2>&1
taskkill /IM node.exe /F >nul 2>&1
taskkill /IM xfchess-tauri.exe /F >nul 2>&1
echo Shutdown complete.
endlocal
