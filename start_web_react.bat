@echo off
chcp 65001 >nul
title XFChess Web UI - Magic Block ER

echo ╔══════════════════════════════════════════════════════════════╗
echo ║           XFChess Web UI - Magic Block ER Test              ║
echo ╚══════════════════════════════════════════════════════════════╝
echo.
echo 🎮 Magic Block ER Features Available:
echo    • Ephemeral Rollups for real-time game state
echo    • Delegation flow for game accounts
echo    • Fast transaction processing
echo    • Seamless session management
echo.
echo 📋 Deployed Program ID:
echo    3D2EnKUfbev1HqU5rMLrZXXwJ4zxbtQ7hUiEYNMcojXP
echo.
echo ═══════════════════════════════════════════════════════════════
echo.

set WEB_DIR=web-solana
set PORT=5173
set URL=http://localhost:%PORT%

REM Check if web-solana directory exists
if not exist %WEB_DIR% (
    echo ❌ Error: %WEB_DIR% directory not found!
    echo    Please run this script from the project root.
    pause
    exit /b 1
)

cd %WEB_DIR%

REM Check if node_modules exists
if not exist node_modules (
    echo 📦 node_modules not found. Running npm install...
    echo    This may take a few minutes...
    echo.
    call npm install
    if errorlevel 1 (
        echo ❌ npm install failed!
        pause
        exit /b 1
    )
    echo ✅ Dependencies installed successfully!
    echo.
) else (
    echo ✅ node_modules found - skipping install
    echo.
)

echo 🚀 Starting React dev server on port %PORT%...
echo    URL: %URL%
echo.
echo ⏳ Waiting for server to start...
echo.

REM Start the dev server in background
start "Vite Dev Server" cmd /c "npm run dev -- --port %PORT%"

REM Wait for server to be ready (check every 2 seconds, max 60 seconds)
echo    Checking server availability...
set /a attempts=0
:check_server
ping -n 3 127.0.0.1 >nul
curl -s -o nul -w "%%{http_code}" %URL% | findstr "200" >nul
if errorlevel 1 (
    set /a attempts+=1
    if %attempts% geq 30 (
        echo ⚠️  Server took too long to start. Opening browser anyway...
        goto open_browser
    )
    goto check_server
)
echo ✅ Server is ready!
echo.

:open_browser
echo 🌐 Opening Chrome browser...
start chrome %URL%

echo.
echo ═══════════════════════════════════════════════════════════════
echo ✅ Web UI is running!
echo    URL: %URL%
echo.
echo Press any key to stop the server and close this window...
echo ═══════════════════════════════════════════════════════════════
pause >nul

echo.
echo 🛑 Stopping dev server...
taskkill /FI "WINDOWTITLE eq Vite Dev Server" /F >nul 2>&1
echo ✅ Server stopped.
echo.
echo Goodbye! 👋
timeout /t 2 >nul
