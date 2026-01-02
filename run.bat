@echo off
REM Set stack size for Bevy's runtime-spawned threads (task pools)
REM Main thread stack is set in .cargo/config.toml
set RUST_MIN_STACK=134217728
REM Use dev mode for fast iteration during development
REM Change to --release if you need optimized performance for testing
cargo run --package xfchess --bin xfchess -- %*
if %ERRORLEVEL% NEQ 0 (
    echo Failed to launch xfchess
)
pause
