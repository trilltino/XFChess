@echo off
echo ========================================
echo XFChess - Live Blockchain Chess
echo ========================================
echo Building with Solana integration...
echo.

REM Build the project with Solana features
echo [1/3] Building XFChess with Solana features...
cargo build --features solana --release-optimized
if %ERRORLEVEL% NEQ 0 (
    echo ERROR: Build failed!
    pause
    exit /b 1
)
echo ✓ Build completed successfully!

REM Check if wallet files exist
echo.
echo [2/3] Checking wallet configuration...
if not exist "player2_wallet.json" (
    echo WARNING: player2_wallet.json not found
    echo Creating test wallet...
    cargo run --bin xfchess --features solana -- --generate-wallet
)

if not exist "playtest_white.json" (
    echo WARNING: playtest_white.json not found
)

if not exist "playtest_black.json" (
    echo WARNING: playtest_black.json not found
)

echo ✓ Wallet files checked!

REM Run the program
echo.
echo [3/3] Starting XFChess with auto move recording...
echo.
echo ========================================
echo IMPORTANT: Auto Move Recording is ACTIVE
echo ========================================
echo Every chess move will be automatically recorded on Solana!
echo Watch for [AUTO_RECORD] logs in console.
echo.
echo Commands:
echo   - Start a game with wager to enable recording
echo   - Make moves normally - they auto-record to blockchain
echo   - Check console for transaction links
echo   - Visit Solana Explorer for live verification
echo.
echo Starting XFChess...
echo.

REM Run the main program with Solana features
cargo run --bin xfchess --features solana --release-optimized

echo.
echo XFChess session ended.
pause
