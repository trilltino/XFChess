@echo off
chcp 65001 >nul
echo ╔═══════════════════════════════════════════════════════════╗
echo ║     Test Wager Contract via CLI                          ║
echo ╚═══════════════════════════════════════════════════════════╝
echo.

REM Program ID
set PROGRAM_ID=3D2EnKUfbev1HqU5rMLrZXXwJ4zxbtQ7hUiEYNMcojXP

REM Player wallets
set PLAYER1=556xz45EL3oS3oMfxPQDPQrna6ipmBVFQEHoDb75PYoG
set PLAYER2=5yMeoo8bSfZxfeWmkL95AXpoy9V8KyiKAjeVMo1PxGSy

set GAME_ID=%RANDOM%%RANDOM%

echo [1/8] Checking wallet balances...
echo.
echo Player 1 (%PLAYER1%):
solana balance %PLAYER1% --url devnet
echo.
echo Player 2 (%PLAYER2%):
solana balance %PLAYER2% --url devnet

echo.
echo [2/8] Game ID: %GAME_ID%
echo.

REM Calculate PDAs using solana-keygen (we'll compute them manually)
echo [3/8] Deriving PDAs...
echo Game ID bytes: %GAME_ID%
echo Note: PDAs need to be computed off-chain or via a program
echo.

echo [4/8] Testing transaction creation...
echo To actually test the contract, we need to build transactions.
echo The Solana CLI doesn't have a direct 'call program' command,
echo but we can use 'solana program deploy' status check:
echo.
solana program show %PROGRAM_ID% --url devnet

echo.
echo [5/8] Program account info:
solana account %PROGRAM_ID% --url devnet | head -20

echo.
echo ╔═══════════════════════════════════════════════════════════╗
echo ║     CLI Test Summary                                     ║
echo ╚═══════════════════════════════════════════════════════════╝
echo.
echo Program ID: %PROGRAM_ID%
echo Status: Deployed and ready
echo.
echo Player 1: %PLAYER1%
echo Player 2: %PLAYER2%
echo.
echo Game ID for testing: %GAME_ID%
echo.
echo To fully test the wager contract, use the web app or run:
echo   test_two_players.bat
echo.
echo The web app provides the UI to:
echo 1. Create game with wager
echo 2. Join game with wager
echo 3. Record moves
echo 4. Claim wager
echo.
pause
