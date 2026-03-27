    @echo off
echo ============================================================
echo Starting XFChess (Local Build - AI ^& PvP)
echo ============================================================
echo.
cd ..
set RUST_LOG=warn
cargo run --bin xfchess
pause
