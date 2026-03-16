@echo off
echo 🎭 Opera Game On-Chain Test Runner
echo ==================================
echo.
echo This script will orchestrate the complete Opera Game test:
echo - Fund player addresses on Solana devnet
echo - Launch two game instances (White and Black)
echo - Monitor auto-recording of all 33 moves
echo - Generate comprehensive results report
echo - Open browser with game results
echo.
echo ⚠️  Make sure you have:
echo - Solana CLI installed
echo - Wallet files: playtest_white.json, playtest_black.json
echo - Internet connection for devnet access
echo.
pause

echo 🚀 Starting Opera Game Test...
echo.

REM Step 1: Build the project
echo [1/6] Building XFChess with Solana features...
cargo build --features solana --bin run_opera_game_test
if %ERRORLEVEL% NEQ 0 (
    echo ERROR: Build failed!
    pause
    exit /b 1
)
echo ✓ Build completed!

REM Step 2: Run the complete test
echo.
echo [2/6] Running complete Opera Game test...
echo This will:
echo   - Fund player addresses
echo   - Launch game instances
echo   - Monitor move recording
echo   - Generate results
echo.
cargo run --bin run_opera_game_test --features solana

echo.
echo 🎉 Opera Game Test Complete!
echo ============================
echo.
echo 📊 Results Summary:
echo - Check opera_game_results.html for detailed game report
echo - All moves should be recorded on Solana devnet
echo - Explorer links available for each transaction
echo.
echo 🔗 Verification:
echo - Solana Explorer: https://explorer.solana.com/?cluster=devnet
echo - Program: 2cUpT4EQXT8D6dWQw6WGfxQm897CFKrvmwpjzCNm1Bix
echo.
echo ✨ The Opera Game is now immortalized on Solana!
echo.
pause
