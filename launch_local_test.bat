@echo off
setlocal EnableDelayedExpansion

echo ==========================================
echo XFChess Local Test Environment Launcher
echo ==========================================
echo.

:: Configuration
set GAME_ID=12345
set WAGER_AMOUNT=0.01
set RPC_URL=https://api.devnet.solana.com
set PROGRAM_ID=AJwEwo74nRiZ3MPKX3XRh92rJaHj5ktPGRiY8kXhVozp

:: Paths
set WEB_UI_PATH=./web-solana
set GAME_EXE=./target/release/xfchess.exe
set DEBUGGER_EXE=./target/release/debugger.exe

:: Check if executables exist
if not exist "%GAME_EXE%" (
    echo [ERROR] Game executable not found: %GAME_EXE%
    echo Please build the game first: cargo build --release
    pause
    exit /b 1
)

:: Create debug directory
if not exist ".local" mkdir .local
if not exist "debug" mkdir debug

:: Step 1: Start Web UI (Player 1 - Host)
echo [1/6] Starting Web UI for Player 1 (Host) on port 5173...
start "Player 1 - Web UI" cmd /k "cd %WEB_UI_PATH% && npm run dev -- --port 5173"
timeout /t 3 /nobreak >nul

:: Step 2: Start Web UI (Player 2 - Joiner)
echo [2/6] Starting Web UI for Player 2 (Joiner) on port 5174...
start "Player 2 - Web UI" cmd /k "cd %WEB_UI_PATH% && npm run dev -- --port 5174"
timeout /t 3 /nobreak >nul

:: Step 3: Open browsers
echo [3/6] Opening browsers...
start chrome "http://localhost:5173" --window-name="Player 1"
timeout /t 2 /nobreak >nul
start chrome "http://localhost:5174" --window-name="Player 2"
timeout /t 2 /nobreak >nul

echo.
echo ==========================================
echo Setup Complete!
echo ==========================================
echo.
echo Instructions:
echo 1. Player 1: Connect wallet, create game with %WAGER_AMOUNT% SOL wager
echo 2. Player 2: Connect different wallet, join game
echo 3. Both: Click "Launch Game" button
echo 4. Game clients will launch automatically
echo.
echo Press any key after both players click "Launch Game"...
pause >nul

:: Step 4: Read launch params (written by Web UI)
echo [4/6] Reading launch parameters...
set PARAMS_FILE=.local\game_launch_params.txt

if not exist "%PARAMS_FILE%" (
    echo [WARNING] Launch params file not found. Using defaults.
    set PLAYER1_SESSION=player1_session_key
    set PLAYER2_SESSION=player2_session_key
    set PLAYER1_NODE_ID=player1_node
    set PLAYER2_NODE_ID=player2_node
) else (
    :: Parse the JSON file (simplified - in real implementation use proper JSON parser)
    for /f "tokens=*" %%a in (%PARAMS_FILE%) do (
        echo Read params: %%a
    )
)

:: Step 5: Launch Game Clients
echo [5/6] Launching game clients...

:: Player 1 (White)
echo Launching Player 1 (White)...
start "Player 1 - Game" cmd /k "%GAME_EXE% ^
    --game-id %GAME_ID% ^
    --player-color white ^
    --rpc-url %RPC_URL% ^
    --p2p-port 5001 ^
    --debug ^
    --log-file debug\player1.log"

timeout /t 2 /nobreak >nul

:: Player 2 (Black)  
echo Launching Player 2 (Black)...
start "Player 2 - Game" cmd /k "%GAME_EXE% ^
    --game-id %GAME_ID% ^
    --player-color black ^
    --rpc-url %RPC_URL% ^
    --p2p-port 5002 ^
    --bootstrap-node player1_node ^
    --debug ^
    --log-file debug\player2.log"

:: Step 6: Launch Transaction Debugger
echo [6/6] Launching transaction debugger...
start "Transaction Debugger" cmd /k "%DEBUGGER_EXE% ^
    --game-id %GAME_ID% ^
    --log-file debug\game_%GAME_ID%.log ^
    --pretty-print"

echo.
echo ==========================================
echo All processes launched!
echo ==========================================
echo Game ID: %GAME_ID%
echo Debugger log: debug\game_%GAME_ID%.log
echo Player 1 log: debug\player1.log
echo Player 2 log: debug\player2.log
echo.
echo Press any key to exit this launcher...
pause >nul
