@echo off
setlocal EnableDelayedExpansion

echo ============================================================
echo  XFChess — Build GitHub Release (Single Player Solana)
echo ============================================================
echo.

cd /d "%~dp0.."

REM Kill running instances so the linker can overwrite the exe
taskkill /IM xfchess-tauri.exe /F >nul 2>&1
taskkill /IM xfchess.exe /F >nul 2>&1

echo [1/4] Compiling xfchess (--features solana, release)...
cargo build --features solana --bin xfchess --release
if %ERRORLEVEL% neq 0 (
    echo ERROR: xfchess release build failed.
    pause
    exit /b 1
)

echo [2/4] Compiling xfchess-tauri (release)...
cargo build -p xfchess-tauri --release
if %ERRORLEVEL% neq 0 (
    echo ERROR: xfchess-tauri release build failed.
    pause
    exit /b 1
)

echo [3/4] Building signing server...
cargo build --release -p backend --bin signing-server
if %ERRORLEVEL% neq 0 (
    echo ERROR: signing-server build failed.
    pause
    exit /b 1
)

echo [4/4] Setting up release structure...
set "RELEASE_DIR=releases\github-release-single"
set "ASSETS_DIR=%RELEASE_DIR%\assets"

REM Clean and recreate release directory
if exist "%RELEASE_DIR%" rmdir /S /Q "%RELEASE_DIR%"
mkdir "%RELEASE_DIR%"
mkdir "%ASSETS_DIR%"
mkdir "%ASSETS_DIR%\bin"
mkdir "%ASSETS_DIR%\game_sounds"
mkdir "%ASSETS_DIR%\models"
mkdir "%ASSETS_DIR%\fonts"

echo [4/4] Copying files...
REM Copy main executable
copy target\release\xfchess-tauri.exe "%RELEASE_DIR%\XFChess.exe"
if %ERRORLEVEL% neq 0 (
    echo ERROR: Failed to copy executable.
    pause
    exit /b 1
)

REM Copy signing server
copy target\release\signing-server.exe "%RELEASE_DIR%\signing-server.exe"

REM Copy Stockfish binary (REQUIRED for AI)
copy assets\bin\stockfish.exe "%ASSETS_DIR%\bin\stockfish.exe"
echo - Stockfish AI binary copied

REM Copy other assets
copy assets\game_sounds\*.mp3 "%ASSETS_DIR%\game_sounds\" >nul 2>&1
copy assets\models\*.glb "%ASSETS_DIR%\models\" >nul 2>&1
copy assets\fonts\*.ttf "%ASSETS_DIR%\fonts\" >nul 2>&1
copy assets\*.webp "%ASSETS_DIR%\" >nul 2>&1
copy assets\*.css "%ASSETS_DIR%\" >nul 2>&1
copy assets\*.js "%ASSETS_DIR%\" >nul 2>&1
copy assets\index.html "%ASSETS_DIR%\" >nul 2>&1

REM Create run.bat for the release
call :create_run_bat

REM Create README.txt
call :create_readme

echo.
echo ============================================================
echo  Single-Player Solana Release Package Complete!
echo ============================================================
echo.
echo Location: %RELEASE_DIR%\
echo.
echo Files included:
echo - XFChess.exe (main game with Solana support)
echo - signing-server.exe (VPS for session signing)
echo - run.bat (launch script)
echo - README.txt (instructions)
echo - assets\bin\stockfish.exe (AI engine)
echo - assets\models\wooden_chess_board.glb (3D pieces)
echo - assets\game_sounds\*.mp3 (audio)
echo - assets\fonts\*.ttf (UI fonts)
echo.
echo To play with another player:
echo 1. Both players extract this package on their computers
echo 2. Both run run.bat
echo 3. Player 1: Create game, share Game ID
echo 4. Player 2: Join with Game ID
echo 5. Both: Exchange Node IDs in Global P2P menu
echo 6. Play!
echo.
pause
exit /b 0

:create_run_bat
(
echo @echo off
echo setlocal EnableDelayedExpansion
echo.
echo echo ============================================================
echo echo  XFChess Solana Single Player
echo ============================================================
echo.
echo cd /d "%%~dp0"
echo.
echo REM --- Kill any running instances ---
echo taskkill /IM xfchess-tauri.exe /F ^>nul 2^>^&1
echo taskkill /IM signing-server.exe /F ^>nul 2^>^&1
echo.
echo REM --- Environment configuration ---
echo set RUST_LOG=info,wgpu_hal=off,wgpu_core=off,wgpu=off,bevy_gltf=off,bevy_image=off,bevy_render=error,bevy_winit=warn,bevy_diagnostic=off,bevy_egui=warn,relay_actor=off,braid_iroh=warn
echo set FEE_PAYER_KEYS=keys\fee-payer.json
echo set SIGNING_PORT=8090
echo set JWT_SECRET=change-me-in-production-32-bytes!!
echo set SOLANA_RPC_URL=https://api.devnet.solana.com
echo set ER_RPC_URL=https://devnet-eu.magicblock.app/
echo set PROGRAM_ID=FVPp29xDtMrh3CrTJNnxDcbGRnMMKuUv2ntqkBRc1uDX
echo set SIGNING_SERVICE_URL=http://127.0.0.1:8090
echo set XFCHESS_SOLANA=1
echo set XFCHESS_WALLET_PORT=7454
echo.
echo REM --- Check fee-payer ---
echo if not exist "keys\fee-payer.json" (
echo     echo [WARNING] Fee-payer key not found!
echo     echo Run: solana-keygen new -o keys\fee-payer.json --no-passphrase
echo     echo Then fund: solana airdrop 2 ^<pubkey^> --url devnet
echo     pause
echo     exit /b 1
echo )
echo.
echo REM --- Check ngrok ---
echo tasklist /FI "IMAGENAME eq ngrok.exe" 2^>NUL ^| find /I /N "ngrok.exe"^>NUL
echo if %%ERRORLEVEL%% neq 0 (
echo     echo [WARNING] ngrok is not running!
echo     echo Start ngrok: ngrok http 8090
echo     echo Then run this script again.
echo     pause
echo     exit /b 1
echo )
echo.
echo echo [1/3] Launching signing server...
echo start "Signing Server" signing-server.exe
echo timeout /t 3 /nobreak ^>nul
echo.
echo echo [2/3] Launching game...
echo start "XFChess" XFChess.exe
echo.
echo echo [3/3] Done!
echo echo.
echo echo ============================================================
echo echo  Game started!
echo ============================================================
echo echo.
echo echo NEXT STEPS:
echo echo 1. Connect wallet when prompted (Phantom/Solflare popup)
echo echo 2. Mode Select ^> Solana Wager Lobby
echo echo 3. Create or Join a game
echo echo 4. Mode Select ^> Global P2P to share Node IDs
echo echo 5. Play!
echo echo.
echo pause
) > "%RELEASE_DIR%\run.bat"
exit /b 0

:create_readme
(
echo ============================================================
echo XFChess Solana - Single Player Release
echo ============================================================
echo.
echo PREREQUISITES:
echo ---------------
echo 1. ngrok installed and in PATH (https://ngrok.com)
echo 2. Solana CLI installed (https://solana.com/docs/intro/installation)
echo 3. A funded devnet fee-payer key
echo.
echo SETUP:
echo --------
echo 1. Create fee-payer key:
echo    solana-keygen new -o keys\fee-payer.json --no-passphrase
echo.
echo 2. Fund the fee-payer:
echo    solana airdrop 2 ^<fee-payer-pubkey^> --url devnet
echo    (If that fails, use: https://faucet.solana.com)
echo.
echo 3. Start ngrok:
echo    ngrok http 8090
echo    (Leave this running in a separate window)
echo.
echo 4. Run the game:
echo    run.bat
echo.
echo PLAYING WITH ANOTHER PLAYER:
echo -----------------------------
echo Both players follow setup steps above on their own computers.
echo.
echo Player 1 (Host):
echo   1. Run run.bat
echo   2. Connect wallet when prompted
echo   3. Mode Select ^> Solana Wager Lobby
echo   4. Click "Create Game"
echo   5. Note the Game ID shown
echo   6. Share Game ID with Player 2
echo   7. Mode Select ^> Global P2P, note your Node ID
echo   8. Share Node ID with Player 2
echo.
echo Player 2 (Joiner):
echo   1. Run run.bat
echo   2. Connect wallet when prompted
echo   3. Mode Select ^> Solana Wager Lobby
echo   4. Enter Game ID from Player 1, click "Join Game"
echo   5. Mode Select ^> Global P2P
echo   6. Enter Player 1's Node ID to connect
echo.
echo GAMEPLAY:
echo ----------
echo - Moves are submitted via your local signing server (no more popups!)
echo - The game state is tracked on-chain via MagicBlock ER
echo - Winner receives the pot at the end
echo.
echo TROUBLESHOOTING:
echo -----------------
echo - If connection fails: Check both ngrok tunnels are running
echo - If moves fail: Check fee-payer has devnet SOL
echo - If wallet won't connect: Check browser popups not blocked
echo.
echo Program ID: FVPp29xDtMrh3CrTJNnxDcbGRnMMKuUv2ntqkBRc1uDX
echo Network: Solana Devnet
echo ER Endpoint: https://devnet-eu.magicblock.app/
echo.
) > "%RELEASE_DIR%\README.txt"
exit /b 0
