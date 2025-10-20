# PowerShell script to run XFChess with increased stack size
# Set minimum stack size to 8MB for all threads to prevent stack overflow
# in Bevy's internal task pools and chess engine deep search

$env:RUST_MIN_STACK = "8388608"
Write-Host "Starting XFChess with 8MB stack size (RUST_MIN_STACK=$env:RUST_MIN_STACK)..." -ForegroundColor Green
cargo run --release
