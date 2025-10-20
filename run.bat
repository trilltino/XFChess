@echo off
REM Set minimum stack size to 8MB for all threads to prevent stack overflow
REM in Bevy's internal task pools and chess engine deep search
set RUST_MIN_STACK=8388608
echo Starting XFChess with 8MB stack size (RUST_MIN_STACK=%RUST_MIN_STACK%)...
cargo run --release
pause
