@echo off
echo ============================================================
echo Starting XFChess (Solana Build - Competitive Mode)
echo ============================================================
echo.
cd ..
cargo run --bin xfchess --features solana
pause
