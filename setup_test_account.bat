@echo off
chcp 65001 >nul
echo ╔═══════════════════════════════════════════════════════════╗
echo ║     Setup Test Account with Devnet SOL                   ║
echo ╚═══════════════════════════════════════════════════════════╝
echo.

REM Check for Solana CLI
where solana >nul 2>&1
if %errorlevel% neq 0 (
    echo ERROR: Solana CLI not found.
    echo Install from: https://docs.solana.com/cli/install
    exit /b 1
)

REM Ensure we're on devnet
echo [1/4] Setting Solana config to devnet...
solana config set --url https://api.devnet.solana.com

echo.
echo [2/4] Generating new test wallet...
set KEYPAIR_PATH=%TEMP%\xfchess_test_wallet_%RANDOM%.json
solana-keygen new --outfile %KEYPAIR_PATH% --no-passphrase --force
if %errorlevel% neq 0 (
    echo ERROR: Failed to generate wallet
    exit /b 1
)

echo.
echo [3/4] Getting wallet address...
for /f "tokens=*" %%a in ('solana-keygen pubkey %KEYPAIR_PATH%') do set WALLET_PUBKEY=%%a
echo Wallet Address: %WALLET_PUBKEY%

echo.
echo [4/4] Requesting devnet SOL airdrop...
solana airdrop 2 %WALLET_PUBKEY% --url https://api.devnet.solana.com
if %errorlevel% neq 0 (
    echo WARNING: Airdrop may have failed (rate limit). Trying again...
    timeout /t 2 >nul
    solana airdrop 2 %WALLET_PUBKEY% --url https://api.devnet.solana.com
)

echo.
echo Checking balance...
solana balance %WALLET_PUBKEY% --url https://api.devnet.solana.com

echo.
echo ╔═══════════════════════════════════════════════════════════╗
echo ║                  Test Account Created                     ║
echo ╚═══════════════════════════════════════════════════════════╝
echo.
echo Wallet Address: %WALLET_PUBKEY%
echo Keypair File:   %KEYPAIR_PATH%
echo.
echo To use this wallet in the web app:
echo 1. Open the web app in your browser
echo 2. Connect wallet (it will use your browser wallet extension)
echo 3. Or import the keypair to your wallet
echo.
echo To save this wallet permanently, copy the keypair file:
echo   copy %KEYPAIR_PATH% .\test_wallet.json
echo.
echo Press any key to exit...
pause >nul
