@echo off
chcp 65001 >nul
echo ╔═══════════════════════════════════════════════════════════╗
echo ║     Setup TWO Test Accounts with Devnet SOL              ║
echo ║     (For testing wager contract with two players)        ║
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
echo [1/6] Setting Solana config to devnet...
solana config set --url https://api.devnet.solana.com

echo.
echo [2/6] Generating Player 1 wallet (White)...
set WALLET1_PATH=%TEMP%\xfchess_player1_%RANDOM%.json
solana-keygen new --outfile %WALLET1_PATH% --no-passphrase --force
if %errorlevel% neq 0 (
    echo ERROR: Failed to generate Player 1 wallet
    exit /b 1
)
for /f "tokens=*" %%a in ('solana-keygen pubkey %WALLET1_PATH%') do set PLAYER1_PUBKEY=%%a
echo Player 1: %PLAYER1_PUBKEY%

echo.
echo [3/6] Generating Player 2 wallet (Black)...
set WALLET2_PATH=%TEMP%\xfchess_player2_%RANDOM%.json
solana-keygen new --outfile %WALLET2_PATH% --no-passphrase --force
if %errorlevel% neq 0 (
    echo ERROR: Failed to generate Player 2 wallet
    exit /b 1
)
for /f "tokens=*" %%a in ('solana-keygen pubkey %WALLET2_PATH%') do set PLAYER2_PUBKEY=%%a
echo Player 2: %PLAYER2_PUBKEY%

echo.
echo [4/6] Requesting devnet SOL for Player 1...
solana airdrop 2 %PLAYER1_PUBKEY% --url https://api.devnet.solana.com
echo.
echo [5/6] Requesting devnet SOL for Player 2...
solana airdrop 2 %PLAYER2_PUBKEY% --url https://api.devnet.solana.com

echo.
echo [6/6] Checking balances...
echo.
echo Player 1 (White):
solana balance %PLAYER1_PUBKEY% --url https://api.devnet.solana.com
echo.
echo Player 2 (Black):
solana balance %PLAYER2_PUBKEY% --url https://api.devnet.solana.com

echo.
echo ╔═══════════════════════════════════════════════════════════╗
echo ║              TWO Test Accounts Created!                   ║
echo ╚═══════════════════════════════════════════════════════════╝
echo.
echo Player 1 (White): %PLAYER1_PUBKEY%
echo   Keypair: %WALLET1_PATH%
echo.
echo Player 2 (Black): %PLAYER2_PUBKEY%
echo   Keypair: %WALLET2_PATH%
echo.
echo To save these wallets permanently:
echo   copy %WALLET1_PATH% .\player1_wallet.json
echo   copy %WALLET2_PATH% .\player2_wallet.json
echo.
echo To test the wager contract:
echo 1. Import Player 1 wallet to browser extension (Phantom/Solflare)
echo 2. Open web app at http://localhost:5173
echo 3. Create game with wager
echo 4. Import Player 2 wallet in another browser/incognito
echo 5. Join the game with matching wager
echo 6. Play and test wager claim!
echo.
echo Or run: test_two_players.bat (automates the browser setup)
echo.
pause
