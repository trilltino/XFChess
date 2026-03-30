@echo off
echo ============================================================
echo  XFChess — Build Solana Multiplayer Release
echo ============================================================
echo.

cd /d "%~dp0.."

REM Kill running instances so the linker can overwrite the exe
taskkill /IM xfchess-tauri.exe /F >nul 2>&1
taskkill /IM xfchess.exe /F >nul 2>&1

echo [1/2] Compiling xfchess (--features solana, release)...
cargo build --features solana --bin xfchess --release
if %ERRORLEVEL% neq 0 (
    echo ERROR: xfchess release build failed.
    pause
    exit /b 1
)

echo [2/2] Compiling xfchess-tauri (release)...
cargo build -p xfchess-tauri --release
if %ERRORLEVEL% neq 0 (
    echo ERROR: xfchess-tauri release build failed.
    pause
    exit /b 1
)

echo [3/3] Copying to releases folder...
if not exist "releases" mkdir releases
copy target\release\xfchess-tauri.exe releases\sol_multiplayer.exe
if %ERRORLEVEL% neq 0 (
    echo ERROR: Failed to copy executable.
    pause
    exit /b 1
)

REM Copy assets if they don't exist
if not exist "releases\assets" xcopy /E /I tauri\wallet-ui\dist releases\assets
if not exist "releases\assets" xcopy /E /I assets releases\assets

echo.
echo Release built: releases\sol_multiplayer.exe
echo.
echo This single exe launches one instance of the Solana multiplayer game.
echo For local testing, run it twice to get two game instances.
echo.
pause
