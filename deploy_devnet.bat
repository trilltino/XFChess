@echo off
REM XFChess Devnet Deploy Script
REM Builds and deploys the xfchess-game program to Solana devnet

setlocal enabledelayedexpansion

echo ======================================
echo XFChess Devnet Deployment
echo ======================================
echo.

REM Check for required tools
echo [1/7] Checking prerequisites...
where solana >nul 2>&1
if %errorlevel% neq 0 (
    echo ERROR: Solana CLI not found. Install from https://docs.solana.com/cli/install
    exit /b 1
)

where cargo >nul 2>&1
if %errorlevel% neq 0 (
    echo ERROR: Rust/Cargo not found. Install from https://rustup.rs/
    exit /b 1
)

REM Check Solana config
echo.
echo [2/7] Checking Solana configuration...
solana config get | findstr "devnet"
if %errorlevel% neq 0 (
    echo WARNING: Solana is not configured for devnet.
    echo Setting config to devnet...
    solana config set --url https://api.devnet.solana.com
    if %errorlevel% neq 0 (
        echo ERROR: Failed to set Solana config
        exit /b 1
    )
)

REM Check wallet balance
echo.
echo [3/7] Checking wallet and balance...
solana balance
if %errorlevel% neq 0 (
    echo WARNING: No wallet configured or balance check failed.
    echo Run 'solana-keygen new' if you need a new wallet.
)

REM Build the program
echo.
echo [4/7] Building program...
cd programs/xfchess-game
cargo build-sbf
if %errorlevel% neq 0 (
    echo ERROR: Program build failed
    cd ../..
    exit /b 1
)
cd ../..

echo.
echo Build successful: target/deploy/xfchess_game.so

REM Get current program ID from lib.rs
echo.
echo [5/7] Checking current program ID...
findstr "declare_id" programs/xfchess-game/src/lib.rs

REM Deploy to devnet
echo.
echo [6/7] Deploying to devnet...
solana program deploy target/deploy/xfchess_game.so --url https://api.devnet.solana.com
if %errorlevel% neq 0 (
    echo ERROR: Program deployment failed
    exit /b 1
)

REM Get new program address
echo.
echo [7/7] Deployment complete!
echo.
echo New Program ID will be displayed above.
echo.
echo NEXT STEPS:
echo 1. Copy the new Program ID from the output above
echo 2. Update programs/xfchess-game/src/lib.rs: declare_id!("NEW_ID")
echo 3. Update crates/solana-chess-client/src/lib.rs: XFCHESS_PROGRAM_ID constant
echo 4. Update src/solana/constants.rs if SOLANA_PROGRAM_ID is defined there
echo 5. Rebuild the program and client with the new ID
echo.
echo To verify deployment:
echo   solana program show PROGRAM_ID --url devnet
echo.

endlocal
