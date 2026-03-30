@echo off
setlocal EnableDelayedExpansion

echo ============================================================
echo  XFChess — Build GitHub Release (with Stockfish AI)
echo ============================================================
echo.

cd /d "%~dp0.."

REM Kill running instances so the linker can overwrite the exe
taskkill /IM xfchess-tauri.exe /F >nul 2>&1
taskkill /IM xfchess.exe /F >nul 2>&1

echo [1/3] Compiling XFChess release binary (with Stockfish AI)...
cargo build --release -p xfchess-tauri
if %ERRORLEVEL% neq 0 (
    echo ERROR: xfchess release build failed.
    pause
    exit /b 1
)

echo [2/3] Setting up release structure...
set "RELEASE_DIR=releases\github-release"
set "ASSETS_DIR=%RELEASE_DIR%\assets"

REM Clean and recreate release directory
if exist "%RELEASE_DIR%" rmdir /S /Q "%RELEASE_DIR%"
mkdir "%RELEASE_DIR%"
mkdir "%ASSETS_DIR%"
mkdir "%ASSETS_DIR%\bin"
mkdir "%ASSETS_DIR%\game_sounds"
mkdir "%ASSETS_DIR%\models"
mkdir "%ASSETS_DIR%\fonts"

echo [3/3] Copying files...
REM Copy main executable
copy target\release\xfchess-tauri.exe "%RELEASE_DIR%\XFChess.exe"
if %ERRORLEVEL% neq 0 (
    echo ERROR: Failed to copy executable.
    pause
    exit /b 1
)

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

echo.
echo ============================================================
echo  GitHub Release Package Complete!
echo ============================================================
echo.
echo Location: %RELEASE_DIR%\
echo.
echo Files included:
echo - XFChess.exe (main game with Stockfish AI)
echo - assets\bin\stockfish.exe (AI engine)
echo - assets\models\wooden_chess_board.glb (3D pieces)
echo - assets\game_sounds\*.mp3 (audio)
echo - assets\fonts\*.ttf (UI fonts)
echo.
echo To deploy to GitHub:
echo 1. Zip the '%RELEASE_DIR%' folder
echo 2. Upload to GitHub Releases
echo 3. Users download, extract, and run XFChess.exe
echo.
echo Features enabled:
echo - Solana blockchain wagering
echo - P2P multiplayer
echo - Stockfish AI opponent
echo - 3D chess board
echo.
pause
