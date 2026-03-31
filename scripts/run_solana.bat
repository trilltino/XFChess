@echo off
cd /d "%~dp0.."

REM Check for Stockfish
if not exist "stockfish.exe" (
    echo Stockfish not found. Downloading...
    powershell -Command "Invoke-WebRequest -Uri 'https://github.com/official-stockfish/Stockfish/releases/download/sf_16/stockfish-windows-x86-64-modern.exe' -OutFile 'stockfish.exe'"
    if not exist "stockfish.exe" (
        echo Failed to download Stockfish. Please download manually and place in project root.
        pause
        exit /b 1
    )
    echo Stockfish downloaded successfully.
)

taskkill /IM xfchess-tauri.exe /F >nul 2>&1
taskkill /IM xfchess.exe /F >nul 2>&1
taskkill /IM signing-server.exe /F >nul 2>&1

set RUST_LOG=info,wgpu_hal=off,wgpu_core=off,wgpu=off,bevy_gltf=off,bevy_image=off,bevy_render=error,bevy_winit=warn,bevy_diagnostic=off,bevy_egui=warn,relay_actor=off,braid_iroh=warn,xfchess::states::main_menu_showcase=off,iroh::address_lookup::pkarr=off,iroh::net_report=off,iroh_quinn_udp=off,iroh_quinn_proto=off,pkarr=off
set FEE_PAYER_KEYS=keys\fee-payer.json
set SIGNING_PORT=8090
set JWT_SECRET=change-me-in-production-32-bytes!!
set SOLANA_RPC_URL=https://api.devnet.solana.com
set ER_RPC_URL=https://devnet-eu.magicblock.app/
set PROGRAM_ID=FVPp29xDtMrh3CrTJNnxDcbGRnMMKuUv2ntqkBRc1uDX
set SIGNING_SERVICE_URL=http://127.0.0.1:8090
set XFCHESS_SOLANA=1

cargo build -p backend --bin signing-server
cargo build --features solana --bin xfchess
cargo build -p xfchess-tauri

start "Signing Server" target\debug\signing-server.exe
timeout /t 3 /nobreak >nul

set XFCHESS_IDENTITY=keys\peer_1.key
set XFCHESS_WALLET_PORT=7454
start "Player 1" target\debug\xfchess-tauri.exe

timeout /t 3 /nobreak >nul

set XFCHESS_IDENTITY=keys\peer_2.key
set XFCHESS_WALLET_PORT=7464
start "Player 2" target\debug\xfchess-tauri.exe

echo.
echo Game instances launched. Close this window to stop all processes.
pause
