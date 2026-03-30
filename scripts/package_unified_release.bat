@echo off
echo ============================================================
echo  XFChess — Package Unified Release (Single EXE + Stockfish)
echo ============================================================
echo.

cd /d "%~dp0.."

set "RELEASE_DIR=releases\XFChess-unified"
set "ASSETS_DIR=%RELEASE_DIR%\assets"

echo [1/3] Creating release structure...
if exist "%RELEASE_DIR%" rmdir /S /Q "%RELEASE_DIR%"
mkdir "%RELEASE_DIR%"
mkdir "%ASSETS_DIR%"
mkdir "%ASSETS_DIR%\bin"
mkdir "%ASSETS_DIR%\game_sounds"
mkdir "%ASSETS_DIR%\models"
mkdir "%ASSETS_DIR%\fonts"

echo [2/3] Copying unified binary + game...
copy target\release\xfchess-tauri.exe "%RELEASE_DIR%\XFChess.exe"
if %ERRORLEVEL% neq 0 (
    echo ERROR: Build the unified binary first with: cargo build -p xfchess-tauri --release
    pause
    exit /b 1
)

REM Copy the Bevy game binary (built with solana feature)
copy target\release\xfchess.exe "%RELEASE_DIR%\xfchess.exe"
if %ERRORLEVEL% neq 0 (
    echo ERROR: Build the game binary first with: cargo build --features solana --bin xfchess --release
    pause
    exit /b 1
)

echo [3/3] Copying assets...
copy assets\bin\stockfish.exe "%ASSETS_DIR%\bin\stockfish.exe"
copy assets\game_sounds\*.mp3 "%ASSETS_DIR%\game_sounds\" >nul 2>&1
copy assets\models\*.glb "%ASSETS_DIR%\models\" >nul 2>&1
copy assets\fonts\*.ttf "%ASSETS_DIR%\fonts\" >nul 2>&1
copy assets\*.webp "%ASSETS_DIR%\" >nul 2>&1
copy assets\*.css "%ASSETS_DIR%\" >nul 2>&1
copy assets\*.js "%ASSETS_DIR%\" >nul 2>&1
copy assets\index.html "%ASSETS_DIR%\" >nul 2>&1

REM Create run.bat
call :create_run_bat

REM Create README
call :create_readme

echo.
echo ============================================================
echo  Unified Release Package Complete!
echo ============================================================
echo.
echo Location: %RELEASE_DIR%\
echo.
echo Files:
echo - XFChess.exe (game + embedded signing server + wallet UI)
echo - assets\bin\stockfish.exe (AI engine)
echo - assets\models\ (3D pieces)
echo - assets\game_sounds\ (audio)
echo - run.bat (launcher)
echo.
echo To play:
echo 1. Start ngrok: ngrok http 8090
echo 2. Run: %RELEASE_DIR%\run.bat
echo 3. Connect wallet, create/join game
echo.
pause
exit /b 0

:create_run_bat
(
echo @echo off
echo echo ============================================================
echo echo  XFChess Unified (Single EXE with Embedded Server)
echo ============================================================
echo.
echo cd /d "%%~dp0"
echo.
echo REM --- Check ngrok ---
echo tasklist /FI "IMAGENAME eq ngrok.exe" 2^>NUL ^| find /I /N "ngrok.exe"^>NUL
echo if %%ERRORLEVEL%% neq 0 (
echo     echo [WARNING] ngrok not running! Start: ngrok http 8090
echo     pause
echo     exit /b 1
echo )
echo.
echo REM --- Check fee-payer ---
echo if not exist "keys\fee-payer.json" (
echo     echo Creating keys folder...
echo     mkdir keys 2^>nul
echo     echo [INFO] You need a fee-payer key:
echo     echo   solana-keygen new -o keys\fee-payer.json --no-passphrase
echo     echo   solana airdrop 2 ^<pubkey^> --url devnet
echo     pause
echo     exit /b 1
echo )
echo.
echo REM --- Launch ---
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
echo echo [1/2] Starting XFChess (embedded signing server)...
echo start "XFChess" XFChess.exe
echo.
echo echo [2/2] Game launched!
echo echo.
echo echo Instructions:
echo echo 1. Connect wallet when browser opens
echo echo 2. Mode Select ^> Solana Wager Lobby
echo echo 3. Create or Join game
echo echo 4. Mode Select ^> Global P2P (share Node ID)
echo echo 5. Play!
echo.
echo pause
) > "%RELEASE_DIR%\run.bat"
exit /b 0

:create_readme
(
echo XFChess Unified Release
echo =======================
echo.
echo Single EXE with embedded signing server for cross-computer play.
echo.
echo PREREQUISITES:
echo - ngrok (https://ngrok.com)
echo - Solana CLI (optional, for key management)
echo.
echo SETUP:
echo 1. Create fee-payer key: solana-keygen new -o keys\fee-payer.json --no-passphrase
echo 2. Fund it: solana airdrop 2 ^<pubkey^> --url devnet
echo 3. Start ngrok: ngrok http 8090
echo 4. Run: run.bat
echo.
echo FEATURES:
echo - Stockfish AI (play against computer)
echo - Solana wagers (bet SOL on games)
echo - P2P multiplayer (play across computers)
echo - Embedded signing server (no separate VPS needed)
echo.
echo Program ID: FVPp29xDtMrh3CrTJNnxDcbGRnMMKuUv2ntqkBRc1uDX
echo Network: Solana Devnet
echo.
) > "%RELEASE_DIR%\README.txt"
exit /b 0
