@echo off
echo ============================================================
echo  XFChess — Compile Only (debug, incremental)
echo ============================================================
echo.
echo  Run this after changing code. The run scripts will pick up
echo  the new binary automatically next time you launch them.
echo.

cd ..

REM Kill running instances so the linker can overwrite the exe
taskkill /IM xfchess-tauri.exe /F >nul 2>&1
taskkill /IM xfchess.exe /F >nul 2>&1

echo [1/2] Compiling xfchess (--features solana)...
cargo build --features solana --bin xfchess
if %ERRORLEVEL% neq 0 (
    echo ERROR: xfchess build failed.
    pause
    exit /b 1
)

echo [2/2] Compiling xfchess-tauri...
cargo build -p xfchess-tauri
if %ERRORLEVEL% neq 0 (
    echo ERROR: xfchess-tauri build failed.
    pause
    exit /b 1
)

echo.
echo Build complete. Run run_multiplayer.bat or run_local.bat to launch.
echo.
pause
