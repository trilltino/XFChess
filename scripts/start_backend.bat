@echo off
setlocal

echo.
echo  ============================================================
echo   XFChess  ^|  Signing Server Launch (Release)
echo  ============================================================
echo.

cd /d "%~dp0.."
set ROOT=%cd%

echo  [!] Starting Rust signing-server in Release mode...
echo  [!] Port: 8090 (Default)
echo.

cd backend
start "XFChess Backend" /D "%ROOT%\backend" cmd /c "cargo run --bin signing-server --release"

REM Auto-start Web Identity Hub
echo  Starting Web Identity Hub (Vite)...
start "XFChess Web Hub" /D "%ROOT%\web-solana" cmd /c "npm run dev"
echo  Web Identity Hub starting...
timeout /t 3 /nobreak >nul
start http://localhost:5173

REM Auto-start Game Client
echo  Launching XFChess Game (Tauri)...
start "" /D "%ROOT%" "%ROOT%\target\release\xfchess-tauri.exe"
echo  Game client starting...

echo.
echo  ============================================================
echo   XFChess  ^|  Distribution Mode (Release)
echo  ============================================================
echo.

pause
endlocal
