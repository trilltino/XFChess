    @echo off
echo ============================================================
echo Starting XFChess (Local Build - AI ^& PvP)
echo ============================================================
echo.
cd /d "%~dp0.."
set RUST_LOG=warn
cargo run --bin xfchess
pause
