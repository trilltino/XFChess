@echo off
setlocal

echo.
echo  ============================================================
echo   XFChess  ^|  Quick Launch (uses pre-built release binaries)
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
echo  [1/2] Cleaning up old instances...
taskkill /IM xfchess.exe        /F >nul 2>&1
taskkill /IM xfchess-tauri.exe  /F >nul 2>&1
taskkill /IM signing-server.exe /F >nul 2>&1
timeout /t 1 /nobreak >nul

REM --- Copy Stockfish to release dir if needed ---
if exist "%ROOT%\stockfish.exe" (
    copy /Y "%ROOT%\stockfish.exe" "%ROOT%\target\release\stockfish.exe" >nul 2>&1
)

REM --- Launch Tauri (serves merged site at localhost:7454, opens Chrome to /onboard) ---
echo  [2/2] Browser will open automatically to localhost:7454/onboard...
"%ROOT%\target\release\xfchess-tauri.exe"

echo.
echo  XFChess process finished.
echo.
endlocal
