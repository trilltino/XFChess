@echo off
echo ============================================================
echo  XFChess Solana Multiplayer  (VPS Signing Flow)
echo ============================================================
echo.
echo  Wallet popup: ONE per player (create/join game only).
echo  Moves, delegation, funding — all handled silently by VPS.
echo.

cd /d "%~dp0.."

REM --- Kill any running instances so the linker can replace the exe ---
taskkill /IM xfchess-tauri.exe /F >nul 2>&1
taskkill /IM xfchess.exe /F >nul 2>&1
taskkill /IM signing-server.exe /F >nul 2>&1

REM ----------------------------------------------------------------
REM  All env vars (inherited by signing-server AND both game clients)
REM ----------------------------------------------------------------
REM  FEE_PAYER_KEYS  path to a Solana JSON keypair file  OR  base58 private key.
REM                  Leave blank for ephemeral keypair (dev only, worthless until funded).
REM  PROGRAM_ID      on-chain XFChess program address.
REM  JWT_SECRET      must be 32+ bytes in production.
REM  SIGNING_PORT    VPS HTTP port (default 8090).
set RUST_LOG=info,wgpu_hal=off,wgpu_core=off,wgpu=off,bevy_gltf=off,bevy_image=off,bevy_render=error,bevy_winit=warn,bevy_diagnostic=off,bevy_egui=warn,relay_actor=off,braid_iroh=warn,xfchess::states::main_menu_showcase=off
set FEE_PAYER_KEYS=keys\fee-payer.json
set SIGNING_PORT=8090
set JWT_SECRET=change-me-in-production-32-bytes!!
set SOLANA_RPC_URL=https://api.devnet.solana.com
set ER_RPC_URL=https://devnet-eu.magicblock.app/
set PROGRAM_ID=FVPp29xDtMrh3CrTJNnxDcbGRnMMKuUv2ntqkBRc1uDX
set SIGNING_SERVICE_URL=http://127.0.0.1:8090
set XFCHESS_SOLANA=1

REM --- Step 1: Ensure ngrok is running ---
echo [1/4] Checking ngrok...
tasklist /FI "IMAGENAME eq ngrok.exe" 2>NUL | find /I /N "ngrok.exe">NUL
if %ERRORLEVEL% neq 0 (
    echo Starting ngrok tunnel...
    start "ngrok tunnel" ngrok http 8090
    timeout /t 3 /nobreak >nul
) else (
    echo ngrok already running.
)

REM --- Step 2: Build signing server ---
echo [2/5] Building signing-server...
cargo build -p backend --bin signing-server
if %ERRORLEVEL% neq 0 (
    echo ERROR: Failed to build signing-server.
    pause
    exit /b 1
)

REM --- Step 3: Build game binary with Solana features ---
echo [3/5] Building xfchess (--features solana)...
cargo build --features solana --bin xfchess
if %ERRORLEVEL% neq 0 (
    echo ERROR: Failed to build xfchess binary.
    pause
    exit /b 1
)

REM --- Step 4: Build Tauri wrapper ---
echo [4/5] Building xfchess-tauri...
cargo build -p xfchess-tauri
if %ERRORLEVEL% neq 0 (
    echo ERROR: Failed to build xfchess-tauri binary.
    pause
    exit /b 1
)

REM --- Step 5: Launch signing server + two Tauri instances ---
echo [5/5] Launching signing-server and two game instances...
echo.

mkdir keys 2>nul

REM Launch signing server (inherits all env vars set above)
start "XFChess Signing Server" target\debug\signing-server.exe

REM Give the signing server a moment to bind its port
timeout /t 3 /nobreak >nul

REM Launch Player 1
set XFCHESS_IDENTITY=keys\peer_1.key
set XFCHESS_WALLET_PORT=7454
start "XFChess Solana - Player 1" target\debug\xfchess-tauri.exe

timeout /t 4 /nobreak >nul

REM Launch Player 2
set XFCHESS_IDENTITY=keys\peer_2.key
set XFCHESS_WALLET_PORT=7464
start "XFChess Solana - Player 2" target\debug\xfchess-tauri.exe

echo.
echo ============================================================
echo  All processes launched.
echo ============================================================
echo.
echo  Signing server : http://127.0.0.1:%SIGNING_PORT%
echo.
echo  FLOW (no repeated popups):
echo   1. Connect wallet once in each Phantom/Solflare popup.
echo   2. Mode Select ^> Solana Wager Lobby.
echo   3. Player 1 creates a game  ^<-- ONE wallet popup
echo      (signs create_game + authorize_session_key together)
echo   4. Player 2 joins that Game ID  ^<-- ONE wallet popup
echo      (signs join_game + authorize_session_key together)
echo   5. Mode Select ^> Global P2P ^> share Node IDs to start.
echo   6. Play — moves submit silently via VPS (no popups).
echo.
echo  Fee-payer key  : keys\fee-payer.json
echo  Fee-payer addr : HSop46SMkLyCSVTizeY1BspuqJn8T2bRNQhUyVgoaC44
echo.
echo  Ensure the fee-payer has devnet SOL before playing:
echo    solana airdrop 2 HSop46SMkLyCSVTizeY1BspuqJn8T2bRNQhUyVgoaC44 --url devnet
echo    OR fund via web: https://faucet.solana.com
echo.
pause
