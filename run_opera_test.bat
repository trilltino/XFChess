@echo off
echo ========================================
echo XFChess Opera Game On-Chain Test
echo ========================================
echo.
echo This will replay the famous 1858 Opera Game
echo (Morphy vs Duke of Brunswick) on Solana devnet
echo using MagicBlock Ephemeral Rollups.
echo.
echo Requirements:
echo - White player wallet: playtest_white.json
echo - Black player wallet: playtest_black.json
echo - Both wallets need ~0.005 SOL on devnet
echo.
pause

cargo run --bin opera_test --features solana --release

pause
