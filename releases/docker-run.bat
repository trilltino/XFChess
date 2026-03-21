@echo off
REM XFChess Docker Run Script for Windows
REM Easy launcher for XFChess in Docker

set IMAGE_NAME=xfchess-iroh:local
set CONTAINER_NAME=xfchess-iroh

REM Create necessary directories
if not exist sessions mkdir sessions
if not exist logs mkdir logs

echo 🎮 XFChess Docker Launcher
echo ==========================
echo.
echo Choose game mode:
echo 1. Single Player (vs AI)
echo 2. Multiplayer - Host Game  
echo 3. Multiplayer - Join Game
echo 4. Load Session File
echo 5. Debug Mode
echo.

set /p choice="Enter your choice (1-5): "

if "%choice%"=="1" (
    echo 🤖 Starting single player game...
    docker run -it --rm ^
        --name %CONTAINER_NAME% ^
        -v "%cd%\sessions:/home/xfchess/sessions" ^
        -v "%cd%\logs:/home/xfchess/logs" ^
        -p 5001:5001 ^
        %IMAGE_NAME% ^
        play --player-color white
) else if "%choice%"=="2" (
    echo 🏠 Starting multiplayer host...
    echo 📝 Share your node ID with your opponent when it appears.
    docker run -it --rm ^
        --name %CONTAINER_NAME% ^
        -v "%cd%\sessions:/home/xfchess/sessions" ^
        -v "%cd%\logs:/home/xfchess/logs" ^
        -p 5001:5001 ^
        %IMAGE_NAME% ^
        play --player-color white --p2p-port 5001
) else if "%choice%"=="3" (
    set /p nodeid="🔗 Enter host's node ID: "
    echo 🔌 Connecting to host...
    docker run -it --rm ^
        --name %CONTAINER_NAME% ^
        -v "%cd%\sessions:/home/xfchess/sessions" ^
        -v "%cd%\logs:/home/xfchess/logs" ^
        -p 5001:5001 ^
        %IMAGE_NAME% ^
        play --player-color black --bootstrap-node %nodeid%
) else if "%choice%"=="4" (
    set /p sessionfile="📁 Enter session file path (relative to sessions\): "
    echo 📂 Loading session...
    docker run -it --rm ^
        --name %CONTAINER_NAME% ^
        -v "%cd%\sessions:/home/xfchess/sessions" ^
        -v "%cd%\logs:/home/xfchess/logs" ^
        -p 5001:5001 ^
        %IMAGE_NAME% ^
        --session-config "/home/xfchess/sessions/%sessionfile%"
) else if "%choice%"=="5" (
    echo 🐛 Starting debug mode...
    docker run -it --rm ^
        --name %CONTAINER_NAME% ^
        -v "%cd%\sessions:/home/xfchess/sessions" ^
        -v "%cd%\logs:/home/xfchess/logs" ^
        -p 5001:5001 ^
        %IMAGE_NAME% ^
        debug
) else (
    echo ❌ Invalid choice. Please run again.
    pause
    exit /b 1
)

echo.
echo 🎮 Game session ended!
echo 📁 Session files saved in: %cd%\sessions\
echo 📋 Logs available in: %cd%\logs\
pause
