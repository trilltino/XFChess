@echo off
chcp 65001 >nul
echo ╔═══════════════════════════════════════════════════════════╗
echo ║       XF Chess - Two Player Test Environment             ║
echo ╚═══════════════════════════════════════════════════════════╝
echo.
echo This script starts the web dev server and opens two browser
echo instances so you can test the end-to-end flow with two wallets.
echo.
echo Player 1 will be the game host (creates the game)
echo Player 2 will join the game (needs to sign with their wallet)
echo.
echo They will connect over the Iroh P2P network.
echo.

:: Find Chrome executable
set CHROME_PATH=
if exist "C:\Program Files\Google\Chrome\Application\chrome.exe" (
    set CHROME_PATH="C:\Program Files\Google\Chrome\Application\chrome.exe"
) else if exist "C:\Program Files (x86)\Google\Chrome\Application\chrome.exe" (
    set CHROME_PATH="C:\Program Files (x86)\Google\Chrome\Application\chrome.exe"
) else if exist "%LOCALAPPDATA%\Google\Chrome\Application\chrome.exe" (
    set CHROME_PATH="%LOCALAPPDATA%\Google\Chrome\Application\chrome.exe"
) else (
    echo ERROR: Chrome not found. Please install Google Chrome.
    exit /b 1
)

echo Found Chrome at: %CHROME_PATH%
echo.

:: Create temp directories for isolated browser profiles
set PLAYER1_PROFILE=%TEMP%\xfchess_player1_%RANDOM%
set PLAYER2_PROFILE=%TEMP%\xfchess_player2_%RANDOM%

mkdir "%PLAYER1_PROFILE%" 2>nul
mkdir "%PLAYER2_PROFILE%" 2>nul

echo Created browser profiles:
echo   Player 1: %PLAYER1_PROFILE%
echo   Player 2: %PLAYER2_PROFILE%
echo.

:: Change to web-solana directory and start dev server in background
echo Starting web dev server...
cd /d "%~dp0web-solana"

:: Start the dev server in a new window
start "XF Chess Dev Server" cmd /k "npm run dev"

:: Wait for server to start
echo Waiting for dev server to start...
timeout /t 5 /nobreak >nul

:: Check if server is running
:wait_for_server
powershell -Command "try { $r = Invoke-WebRequest -Uri 'http://localhost:5173' -Method HEAD -TimeoutSec 2; exit 0 } catch { exit 1 }"
if errorlevel 1 (
    echo Still waiting for server...
    timeout /t 2 /nobreak >nul
    goto wait_for_server
)

echo Dev server is ready!
echo.

:: Launch Player 1 browser (Game Host)
echo Launching Player 1 (Game Host)...
start "Player 1 - Game Host" %CHROME_PATH% ^
    --user-data-dir="%PLAYER1_PROFILE%" ^
    --new-window ^
    --window-size=1400,900 ^
    --window-position=0,0 ^
    --app="http://localhost:5173" ^
    --disable-background-timer-throttling ^
    --disable-backgrounding-occluded-windows

timeout /t 2 /nobreak >nul

:: Launch Player 2 browser (Game Joiner)
echo Launching Player 2 (Game Joiner)...
start "Player 2 - Game Joiner" %CHROME_PATH% ^
    --user-data-dir="%PLAYER2_PROFILE%" ^
    --new-window ^
    --window-size=1400,900 ^
    --window-position=700,0 ^
    --app="http://localhost:5173" ^
    --disable-background-timer-throttling ^
    --disable-backgrounding-occluded-windows

echo.
echo ╔═══════════════════════════════════════════════════════════╗
echo ║                   Test Setup Complete                     ║
echo ╚═══════════════════════════════════════════════════════════╝
echo.
echo INSTRUCTIONS:
echo 1. In Player 1 window: Connect wallet and create a game
echo 2. Copy the Game ID from Player 1's lobby
echo 3. In Player 2 window: Connect a DIFFERENT wallet
echo 4. Enter the Game ID and click Join
echo 5. Player 2 will be prompted to sign a session
echo 6. Both players can now launch the native game
echo.
echo The Iroh P2P network will connect the players.
echo.
echo Press any key to close this window (servers will keep running)
echo You can close the dev server window separately when done.
pause >nul

:: Cleanup browser profiles on exit
rmdir /s /q "%PLAYER1_PROFILE%" 2>nul
rmdir /s /q "%PLAYER2_PROFILE%" 2>nul
