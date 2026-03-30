@echo off
setlocal EnableDelayedExpansion

echo ============================================================
echo  XFChess Solana Single Player (Cross-Computer Play)
echo ============================================================
echo.
echo  For playing against another player on a different computer.
echo.
echo  Your computer runs:
echo   - Signing server (signs moves after wallet auth)
echo   - ngrok tunnel (for P2P connectivity)
echo   - Game client (you)
echo.
echo  Other player's computer runs the same setup.
echo.

cd /d "%~dp0.."

REM --- Kill any running instances so the linker can replace the exe ---
taskkill /IM xfchess-tauri.exe /F >nul 2>&1
taskkill /IM xfchess.exe /F >nul 2>&1
taskkill /IM signing-server.exe /F >nul 2>&1

REM ----------------------------------------------------------------
REM  Environment configuration
REM ----------------------------------------------------------------
set RUST_LOG=info,wgpu_hal=off,wgpu_core=off,wgpu=off,bevy_gltf=off,bevy_image=off,bevy_render=error,bevy_winit=warn,bevy_diagnostic=off,bevy_egui=warn,relay_actor=off,braid_iroh=warn,xfchess::states::main_menu_showcase=off
set FEE_PAYER_KEYS=keys\fee-payer.json
set SIGNING_PORT=8090
set JWT_SECRET=change-me-in-production-32-bytes!!
set SOLANA_RPC_URL=https://api.devnet.solana.com
set ER_RPC_URL=https://devnet-eu.magicblock.app/
set PROGRAM_ID=FVPp29xDtMrh3CrTJNnxDcbGRnMMKuUv2ntqkBRc1uDX
set SIGNING_SERVICE_URL=http://127.0.0.1:8090
set XFCHESS_SOLANA=1

REM --- Player identity (unique per computer) ---
if not exist "keys\player.key" (
    echo [INFO] Generating new player identity...
    mkdir keys 2>nul
    echo player_identity_%RANDOM%%RANDOM% > keys\player.key
)
set /p XFCHESS_IDENTITY=<keys\player.key
set XFCHESS_IDENTITY=keys\player.key
set XFCHESS_WALLET_PORT=7454

REM --- Step 1: Check fee-payer is funded ---
echo [1/5] Checking fee-payer key...
if not exist "keys\fee-payer.json" (
    echo [WARNING] Fee-payer key not found at keys\fee-payer.json
    echo Creating placeholder - you MUST fund this before playing:
    echo   solana-keygen new -o keys\fee-payer.json --no-passphrase
    echo   solana airdrop 2 ^<fee-payer-pubkey^> --url devnet
    echo.
    pause
)

REM --- Step 2: Ensure ngrok is running ---
echo [2/5] Checking ngrok...
tasklist /FI "IMAGENAME eq ngrok.exe" 2>NUL | find /I /N "ngrok.exe">NUL
if %ERRORLEVEL% neq 0 (
    echo Starting ngrok tunnel on port 8090...
    echo [IMPORTANT] Other player will need your ngrok URL for P2P connection
    start "ngrok tunnel" ngrok http 8090
    timeout /t 3 /nobreak >nul
) else (
    echo ngrok already running.
)

REM --- Step 3: Build signing server ---
echo [3/5] Building signing-server...
cargo build -p backend --bin signing-server
if %ERRORLEVEL% neq 0 (
    echo ERROR: Failed to build signing-server.
    pause
    exit /b 1
)

REM --- Step 4: Build game binary with Solana features ---
echo [4/5] Building xfchess (--features solana)...
cargo build --features solana --bin xfchess
if %ERRORLEVEL% neq 0 (
    echo ERROR: Failed to build xfchess binary.
    pause
    exit /b 1
)

REM --- Step 5: Build Tauri wrapper ---
echo [5/5] Building xfchess-tauri...
cargo build -p xfchess-tauri
if %ERRORLEVEL% neq 0 (
    echo ERROR: Failed to build xfchess-tauri binary.
    pause
    exit /b 1
)

REM --- Launch ---
echo.
echo ============================================================
echo  Launching signing server and game client...
echo ============================================================
echo.

REM Launch signing server (inherits all env vars set above)
start "XFChess Signing Server" target\debug\signing-server.exe

REM Give the signing server a moment to bind its port
timeout /t 3 /nobreak >nul

REM Launch Player Instance
start "XFChess Solana - Player" target\debug\xfchess-tauri.exe

echo.
echo ============================================================
echo  Game launched!
echo ============================================================
echo.
echo  TO PLAY WITH ANOTHER PLAYER:
echo.
echo  1. Ensure your fee-payer is funded:
echo     solana balance ^<fee-payer-pubkey^> --url devnet
echo.
echo  2. Share your Node ID with the other player:
echo     - In game: Mode Select ^> Global P2P ^> Your Node ID is shown
echo     - Or check ngrok: http://127.0.0.1:4040/status
echo.
echo  3. To CREATE a game:
echo     - Mode Select ^> Solana Wager Lobby ^> Create Game
echo     - ONE wallet popup (signs create_game + authorize session)
echo     - Share the Game ID with other player
echo.
echo  4. To JOIN a game:
echo     - Mode Select ^> Solana Wager Lobby ^> Join Game
echo     - Enter Game ID from other player
echo     - ONE wallet popup (signs join_game + authorize session)
echo.
echo  5. Connect P2P:
echo     - Both: Mode Select ^> Global P2P
echo     - Exchange Node IDs and connect
echo.
echo  6. Play - moves submit via your local signing server (no more popups!)
echo.
echo  Fee-payer: keys\fee-payer.json
echo  Signing server: http://127.0.0.1:%SIGNING_PORT%
echo.
pause
