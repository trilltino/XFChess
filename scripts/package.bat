@echo off
cd /d "%~dp0.."

taskkill /IM xfchess-tauri.exe /F >nul 2>&1
taskkill /IM xfchess.exe /F >nul 2>&1

cargo build --features solana --bin xfchess --release
cargo build -p xfchess-tauri --release
cargo build --release -p backend --bin signing-server

set "RELEASE_DIR=releases\xfchess"
set "ASSETS_DIR=%RELEASE_DIR%\assets"

if exist "%RELEASE_DIR%" rmdir /S /Q "%RELEASE_DIR%"
mkdir "%RELEASE_DIR%"
mkdir "%ASSETS_DIR%"
mkdir "%ASSETS_DIR%\bin"
mkdir "%ASSETS_DIR%\game_sounds"
mkdir "%ASSETS_DIR%\models"
mkdir "%ASSETS_DIR%\fonts"

copy target\release\xfchess-tauri.exe "%RELEASE_DIR%\XFChess.exe"
copy target\release\signing-server.exe "%RELEASE_DIR%\signing-server.exe"
copy assets\bin\stockfish.exe "%ASSETS_DIR%\bin\stockfish.exe"
copy assets\game_sounds\*.mp3 "%ASSETS_DIR%\game_sounds\" >nul 2>&1
copy assets\models\*.glb "%ASSETS_DIR%\models\" >nul 2>&1
copy assets\fonts\*.ttf "%ASSETS_DIR%\fonts\" >nul 2>&1
copy assets\*.webp "%ASSETS_DIR%\" >nul 2>&1

(
echo @echo off
echo cd /d "%%~dp0"
echo set FEE_PAYER_KEYS=keys\fee-payer.json
echo set SIGNING_PORT=8090
echo set JWT_SECRET=change-me-in-production-32-bytes!!
echo set SOLANA_RPC_URL=https://api.devnet.solana.com
echo set ER_RPC_URL=https://devnet-eu.magicblock.app/
echo set PROGRAM_ID=FVPp29xDtMrh3CrTJNnxDcbGRnMMKuUv2ntqkBRc1uDX
echo set SIGNING_SERVICE_URL=http://127.0.0.1:8090
echo set XFCHESS_SOLANA=1
echo set XFCHESS_WALLET_PORT=7454
echo start "XFChess" XFChess.exe
) > "%RELEASE_DIR%\run.bat"

echo.
echo Release ready: %RELEASE_DIR%\n
