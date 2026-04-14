@echo off
setlocal

echo.
echo  ============================================================
echo   XFChess  ^|  Tournament Host (Release)
echo  ============================================================
echo.

cd /d "%~dp0.."
set ROOT=%cd%

REM --- Admin Configuration ---
echo.
echo  Tournament Configuration:
echo.
echo  1. Tournament Size:
echo     [1] 4 Players (Current - 3 matches)
echo     [2] 8 Players (Requires code update - 7 matches)
echo     [3] 16 Players (Requires code update - 15 matches)
echo.
set /p SIZE="Select tournament size (1-3): "

echo.
echo  2. External Access (Ngrok):
echo     [1] Local only (players on same network only)
echo     [2] Ngrok tunnel (external players can join)
echo.
set /p NGROK="Select access mode (1-2): "

if %NGROK==2 (
    echo.
    echo  Enter your ngrok authtoken (get from https://dashboard.ngrok.com/get-started/your-authtoken):
    set /p NGROK_TOKEN="Ngrok Auth Token: "
)

REM --- Kill any running instances ---
echo.
echo [1/6] Cleaning up old instances...
taskkill /IM xfchess.exe        /F >nul 2>&1
taskkill /IM xfchess-tauri.exe  /F >nul 2>&1
taskkill /IM signing-server.exe /F >nul 2>&1
taskkill /IM ngrok.exe           /F >nul 2>&1
timeout /t 1 /nobreak >nul

REM --- Ensure Stockfish is available ---
echo.
echo [2/6] Ensuring Stockfish is available...
if not exist "%ROOT%\stockfish.exe" (
    echo  WARNING: stockfish.exe not found! AI will not work.
) else (
    if not exist "%ROOT%\target\release" mkdir "%ROOT%\target\release"
    copy /Y "%ROOT%\stockfish.exe" "%ROOT%\target\release\stockfish.exe" >nul
)

REM --- Build backend in release mode ---
echo.
echo [3/6] Building signing-server in Release mode...
cd backend
cargo build --bin signing-server --release --quiet
if %ERRORLEVEL% neq 0 (
    echo  ERROR: Backend build failed.
    pause
    exit /b 1
)

REM --- Start signing server (tournament backend) ---
echo.
echo [4/6] Starting Tournament Backend Server...
echo  Port: 8090
echo  Tournament API: http://localhost:8090
echo  Admin endpoints: http://localhost:8090/admin/tournament/*
start "XFChess Tournament Server" /D "%ROOT%\backend" cmd /c "cargo run --bin signing-server --release"
timeout /t 3 /nobreak >nul

REM --- Start ngrok if requested ---
if %NGROK==2 (
    echo.
    echo [5/6] Starting ngrok tunnel for external access...
    echo  Configuring ngrok with auth token...
    ngrok config add-authtoken %NGROK_TOKEN%
    echo  Starting ngrok tunnel on port 8090...
    start "XFChess Ngrok Tunnel" ngrok http 8090 --log=stdout
    timeout /t 3 /nobreak >nul
    echo  Ngrok tunnel started!
    echo  Check the ngrok window for the public URL.
    echo  Share this URL with players.
) else (
    echo.
    echo [5/6] Skipping ngrok (local access only)
)

REM --- Display summary ---
echo.
echo [6/6] Tournament Host Ready!
echo.
echo  ============================================================
echo   Configuration Summary
echo  ============================================================
echo.
if %SIZE%==1 (
    echo  Tournament Size: 4 Players (3 matches)
) else if %SIZE%==2 (
    echo  Tournament Size: 8 Players (CODE UPDATE REQUIRED)
    echo  WARNING: 8-player mode not implemented yet!
) else if %SIZE%==3 (
    echo  Tournament Size: 16 Players (CODE UPDATE REQUIRED)
    echo  WARNING: 16-player mode not implemented yet!
)

if %NGROK==2 (
    echo  Access Mode: External (via ngrok)
    echo  Check ngrok window for public URL
) else (
    echo  Access Mode: Local only
    echo  Players must be on same network
)
echo.
echo  ============================================================
echo   Tournament Server Running!
echo  ============================================================
echo.
echo  API Endpoints:
echo    GET    /tournaments                    - List tournaments
echo    POST   /admin/tournament/create       - Create tournament
echo    POST   /tournament/:id/join           - Join tournament
echo    GET    /tournament/:id/bracket         - View bracket
echo.
echo  Players connect via P2P (Iroh) - no port forwarding needed!
echo  Security: TLS encrypted, node ID authentication
echo.
echo  Press Ctrl+C to stop the server.
echo.

pause
endlocal
