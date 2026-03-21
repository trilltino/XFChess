@echo off
echo Starting XFChess - Iroh Networking Version
echo.
echo Choose game mode:
echo 1. Single Player (vs AI)
echo 2. Multiplayer - Host Game
echo 3. Multiplayer - Join Game
echo 4. Load Session File
echo.
set /p choice="Enter your choice (1-4): "

if "%choice%"=="1" (
    echo Starting single player game...
    XFChess-Iroh.exe play --player-color white
) else if "%choice%"=="2" (
    echo Starting multiplayer host...
    echo Share your node ID with your opponent when it appears.
    XFChess-Iroh.exe play --player-color white --p2p-port 5001
) else if "%choice%"=="3" (
    set /p nodeid="Enter host's node ID: "
    echo Connecting to host...
    XFChess-Iroh.exe play --player-color black --bootstrap-node %nodeid%
) else if "%choice%"=="4" (
    set /p sessionfile="Enter session file path: "
    echo Loading session...
    XFChess-Iroh.exe --session-config %sessionfile%
) else (
    echo Invalid choice. Please run again.
    pause
)

if %errorlevel% neq 0 (
    echo.
    echo Game exited with error. Check README.md for troubleshooting.
    pause
)
