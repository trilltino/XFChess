@echo off
setlocal

echo ============================================
echo XFChess E2E Test - Manual Mode
echo ============================================
echo.
echo This script will:
echo   1. Start Web UI for session creation at http://localhost:5173
echo   2. Run Player 1 EXE
echo   3. Prompt for Player 1's Node ID
echo   4. Run Player 2 EXE with bootstrap_node
echo.
echo Program ID: 3D2EnKUfbev1HqU5rMLrZXXwJ4zxbtQ7hUiEYNMcojXP
echo ER Endpoint: https://devnet-eu.magicblock.app
echo.
echo ============================================
echo.

echo Checking prerequisites...

node --version >nul 2>&1
if errorlevel 1 goto :nonode
echo Node.js found
goto :checkexe

:nonode
echo ERROR: Node.js not found! Please install it first.
pause
exit /b 1

:checkexe
if exist "target\release\xfchess.exe" goto :skipbuild

echo Building XFChess binary with Solana/ER support (this may take a while)...
cargo build --release
if errorlevel 1 goto :buildfail
echo Rust binary built successfully!
goto :verifyexe

:buildfail
echo ERROR: Rust build failed! Please check for compilation errors.
pause
exit /b 1

:skipbuild
echo Found existing xfchess.exe - skipping build

:verifyexe
if exist "target\release\xfchess.exe" goto :havedirs
echo ERROR: xfchess.exe not found at target\release\xfchess.exe
pause
exit /b 1

:havedirs
if exist web-solana goto :startui
echo ERROR: web-solana directory not found!
pause
exit /b 1

:startui
echo Starting Player 1 Web UI on port 5173...
cd web-solana

if exist node_modules goto :depsdone
echo Installing dependencies...
call npm install >nul 2>&1
if errorlevel 1 goto :depsfail
goto :depsdone

:depsfail
echo ERROR: Failed to install dependencies
pause
exit /b 1

:depsdone

echo.
echo ============================================
echo SELECT MODE:
echo ============================================
echo.
echo 1. Single Player (Player 1 only on port 5173)
echo 2. Two Players (Player 1 on 5173, Player 2 on 5174)
echo.
set /p MODE="Enter choice (1 or 2): "

if "%MODE%"=="1" goto :singleplayer
if "%MODE%"=="2" goto :twoplayers

echo Invalid choice, defaulting to single player...
goto :singleplayer

:singleplayer
echo.
echo Starting Web UI on port 5173 (Single Player Mode)...
start "Player 1 Web UI" cmd /c "npm run dev -- --port 5173"
cd ..
echo Web UI started at http://localhost:5173
echo.
timeout /t 3 >nul
echo Opening browser...
start chrome "http://localhost:5173" --new-window 2>nul
echo Browser opened.
echo.
goto :instructions

:twoplayers
echo.
echo Starting Player 1 Web UI on port 5173...
start "Player 1 Web UI" cmd /c "npm run dev -- --port 5173"
cd ..
echo Player 1 Web UI started at http://localhost:5173
echo.

echo Starting Player 2 Web UI on port 5174...
cd web-solana
start "Player 2 Web UI" cmd /c "npm run dev -- --port 5174"
cd ..
echo Player 2 Web UI started at http://localhost:5174
echo.

timeout /t 3 >nul
echo Opening Player 1 browser...
start chrome "http://localhost:5173" --new-window 2>nul
timeout /t 2 >nul
echo Opening Player 2 browser...
start chrome "http://localhost:5174" --new-window 2>nul
echo Browsers opened for both players
echo.
goto :instructions_twoplayer

:instructions

:instructions

:instructions_twoplayer
echo ============================================
echo SETUP INSTRUCTIONS:
echo ============================================
echo.
echo 1. Get devnet SOL from https://faucet.solana.com/
echo.
echo 2. Player 1 (White) - Browser window (port 5173):
echo    a. Connect your wallet (supports Phantom, Solflare, etc.)
echo    b. Click "Host Game" 
echo    c. Click "Create Game Session"
echo    d. Click "Launch Game" to download session config
echo    e. Save to: e2e_sessions\player1_session.json
echo    f. Create game on-chain (use game UI)
echo.

if "%MODE%"=="2" goto :twoplayer_instructions
goto :singleplayer_instructions

:singleplayer_instructions
echo ============================================
echo SINGLE PLAYER MODE - You can:
echo    • Play against the AI (PvAI mode)
echo    • Or wait for another player to join online
echo ============================================
echo.
goto :continue_setup

:twoplayer_instructions
echo 3. Player 2 (Black) - RIGHT browser window (port 5174):
echo    a. Connect your wallet
echo    b. Click "Join Game"
echo    c. Enter Game ID from Player 1
echo    d. Click "Create Game Session"
echo    e. Click "Launch Game" to download session config
echo    f. Save to: e2e_sessions\player2_session.json
echo    g. Join game on-chain
echo.

:continue_setup
echo ============================================
echo.

if exist e2e_sessions goto :havesessions
mkdir e2e_sessions

:havesessions
echo ============================================
echo COMPLETE THE SETUP ABOVE BEFORE CONTINUING
echo ============================================
echo.
echo Press any key when BOTH players have downloaded their session configs...
pause >nul

echo.
echo Checking for session files...

if exist "e2e_sessions\player1_session.json" goto :p1ok
echo WARNING: Player 1 session not found at e2e_sessions\player1_session.json
echo Please save the downloaded config to this location.
pause

:p1ok

REM Only check for Player 2 session if in two-player mode
if not "%MODE%"=="2" goto :p2skip

if exist "e2e_sessions\player2_session.json" goto :p2ok
echo WARNING: Player 2 session not found at e2e_sessions\player2_session.json
echo Please save the downloaded config to this location.
pause

:p2ok
:p2skip
echo.
echo ============================================
echo STARTING PLAYER 1 (White)
echo ============================================
echo.
echo Starting Player 1 EXE...
echo IMPORTANT: Copy your Node ID from the game window when it appears!
echo.

if exist "e2e_sessions\player1_session.json" goto :p1withconfig
goto :p1noconfig

:p1withconfig
copy /Y "e2e_sessions\player1_session.json" "session_config.json" >nul
start "Player 1 - XFChess" "target\release\xfchess.exe" --session-config session_config.json
goto :p1done

:p1noconfig
start "Player 1 - XFChess" "target\release\xfchess.exe"

:p1done
echo Player 1 EXE launched
echo.

echo ============================================
echo NODE ID EXCHANGE
echo ============================================
echo.
echo Player 1: Check the game window for your Node ID
echo It should be displayed in the UI after the game starts.
echo.

set /p P1_NODE_ID="Enter Player 1's Node ID: "

if "%P1_NODE_ID%"=="" goto :nonodeid
goto :gotnodeid

:nonodeid
echo ERROR: No Node ID provided!
pause
exit /b 1

:gotnodeid
echo.
echo Player 1 Node ID: %P1_NODE_ID%
echo.

REM Skip Player 2 if in single-player mode
if not "%MODE%"=="2" goto :singleplayer_done

echo ============================================
echo STARTING PLAYER 2 (Black)
echo ============================================
echo.
echo Starting Player 2 EXE with bootstrap_node=%P1_NODE_ID%
echo.

if exist "e2e_sessions\player2_session.json" goto :p2withconfig
goto :p2noconfig

:p2withconfig
copy /Y "e2e_sessions\player2_session.json" "session_config.json" >nul
start "Player 2 - XFChess" "target\release\xfchess.exe" --session-config session_config.json --bootstrap-node %P1_NODE_ID%
goto :p2done

:p2noconfig
start "Player 2 - XFChess" "target\release\xfchess.exe" --bootstrap-node %P1_NODE_ID%

:p2done
echo Player 2 EXE launched
echo.

echo ============================================
echo ALL SYSTEMS RUNNING!
echo ============================================
echo.
echo Both players should now be connected!
goto :finish

:finish
echo.
echo To stop all processes:
echo    - Close the game windows
echo    - Close the Web UI window
echo    - Close this window
echo.
echo Press any key to exit this script (games will keep running)...
pause >nul

taskkill /FI "WINDOWTITLE eq Player 1 Web UI*" /F >nul 2>&1
if "%MODE%"=="2" taskkill /FI "WINDOWTITLE eq Player 2 Web UI*" /F >nul 2>&1

echo Script complete. Games are still running!
