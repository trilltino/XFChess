@echo off
echo ============================================================
echo Starting XFChess Multiplayer Test (Two Instances)
echo ============================================================
echo.

cd ..

REM Build the game binary once
echo [1/2] Building XFChess game binary...
set RUST_LOG=info
cargo build --bin xfchess
if %ERRORLEVEL% neq 0 (
    echo ERROR: Failed to build game binary.
    pause
    exit /b 1
)

REM Launch two instances with unique identities
echo [2/2] Launching two game instances...
echo.
echo  Instance 1 = Host  (identity: keys\peer_1.key)
echo  Instance 2 = Join  (identity: keys\peer_2.key)
echo.

set XFCHESS_IDENTITY=keys\peer_1.key
mkdir keys 2>nul
start "XFChess - Player 1 (Host)" target\debug\xfchess.exe
timeout /t 3 /nobreak >nul
set XFCHESS_IDENTITY=keys\peer_2.key
mkdir keys 2>nul
start "XFChess - Player 2 (Join)" target\debug\xfchess.exe

echo Both instances launched. Use Host/Join in each game's menu.
pause
