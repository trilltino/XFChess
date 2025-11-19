@echo off
REM Set stack size for Bevy's runtime-spawned threads (task pools)
REM Main thread stack is set in .cargo/config.toml
set RUST_MIN_STACK=134217728
REM Always build in release mode before running to ensure changes are applied
cargo build --release
if %ERRORLEVEL% EQU 0 (
    cargo run --release
) else (
    echo Build failed, not running
)
pause
