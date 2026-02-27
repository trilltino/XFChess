@echo off
REM Deploy XFChess Game Program with Magic Block Ephemeral Rollups Support
REM This script builds and deploys the program to Solana devnet with ER delegation enabled

echo ========================================
echo XFChess Magic Block ER Deployment
echo ========================================
echo.

REM Check if we're in the right directory
if not exist "programs\xfchess-game\Cargo.toml" (
    echo Error: Must run from project root directory
    exit /b 1
)

REM Set Solana cluster to devnet
echo Setting Solana cluster to devnet...
solana config set --url https://api.devnet.solana.com

REM Verify wallet has funds
echo.
echo Checking wallet balance...
solana balance

REM Build the program with magicblock feature
echo.
echo Building program with Magic Block ER feature...
cd programs\xfchess-game
anchor build -- --features magicblock

if %errorlevel% neq 0 (
    echo Error: Build failed
    exit /b 1
)

cd ..\..

REM Deploy the program
echo.
echo Deploying to devnet...
anchor deploy --provider.cluster devnet --program-name xfchess_game

if %errorlevel% neq 0 (
    echo Error: Deployment failed
    exit /b 1
)

echo.
echo ========================================
echo Deployment Complete!
echo ========================================
echo.
echo Program deployed with Magic Block ER support.
echo Features enabled:
echo   - delegate_game: Delegate Game PDA to ER
echo   - undelegate_game: Commit and undelegate from ER
echo.
echo Next steps:
echo   1. Update web-solana/src/idl/xfchess_game.json with new IDL
echo   2. Run: anchor idl init --filepath target/idl/xfchess_game.json ^<PROGRAM_ID^>
echo   3. Test delegation flow
echo.
pause
