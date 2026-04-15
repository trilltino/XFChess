@echo off
setlocal

echo.
echo  ============================================================
echo   XFChess  ^|  Dual Instance Launcher (Local Multiplayer)
echo  ============================================================
echo.

cd /d "%~dp0.."
set ROOT=%cd%

REM --- Check that binaries exist ---
if not exist "%ROOT%\target\release\xfchess-tauri.exe" (
    echo  ERROR: xfchess-tauri.exe not found. Run start_xfchess.bat first to build.
    pause
    exit /b 1
)
if not exist "%ROOT%\target\release\xfchess.exe" (
    echo  ERROR: xfchess.exe not found. Run start_xfchess.bat first to build.
    pause
    exit /b 1
)

REM --- Kill running instances ---
echo  [1/3] Cleaning up old instances...
taskkill /IM xfchess.exe        /F >nul 2>&1
taskkill /IM xfchess-tauri.exe  /F >nul 2>&1
taskkill /IM signing-server.exe /F >nul 2>&1
taskkill /IM signing-server-http.exe /F >nul 2>&1
taskkill /IM vps_admin.exe /F >nul 2>&1
timeout /t 1 /nobreak >nul

REM --- Copy Stockfish to release dir if needed ---
if exist "%ROOT%\stockfish.exe" (
    copy /Y "%ROOT%\stockfish.exe" "%ROOT%\target\release\stockfish.exe" >nul 2>&1
)

REM --- Backend URL is now compiled into binary via build.rs ---
echo.
echo [Config] Backend URL is compiled into binary (backend_url.txt).
echo  Edit backend_url.txt and rebuild to change.
echo.

REM --- Start Local Backend ---
echo.
echo  [2/3] Starting Backend HTTP Server (port 8090)...
start "XFChess Backend" /D "%ROOT%\backend" cmd /c "set SIGNING_SERVICE_URL=http://localhost:8090 && target\release\signing-server-http.exe"
echo  Backend server starting...
timeout /t 3 /nobreak >nul

REM --- Launch First Instance (Player 1) ---
echo.
echo  [3/3] Launching TWO game instances...
echo.
echo  Starting Player 1 (Port 5001)...
start "XFChess Player 1" /D "%ROOT%" cmd /c "target\release\xfchess.exe --p2p-port 5001"
timeout /t 2 /nobreak >nul

REM --- Launch Second Instance (Player 2) ---
echo  Starting Player 2 (Port 5002)...
start "XFChess Player 2" /D "%ROOT%" cmd /c "target\release\xfchess.exe --p2p-port 5002"

echo.
echo  ============================================================
echo   Two XFChess instances launched!
echo.
echo   Player 1: P2P Port 5001
echo   Player 2: P2P Port 5002
echo   Backend:  http://localhost:8090
echo.
echo   Use P2P connection in-game to connect the two players.
echo  ============================================================
echo.

endlocal
